use crate::pd_func_caller;
use alloc::{
    boxed::Box,
    format,
    rc::{Rc, Weak},
    string::String,
};
use anyhow::{anyhow, ensure, Error, Result};
use core::{cell::RefCell, convert::TryInto, mem::ManuallyDrop, ptr};
use crankstart_sys::{
    accessReply, ctypes, playdate_http, playdate_network, playdate_tcp, HTTPConnection,
    HTTPConnectionCallback as PdHTTPConnectionCallback, HTTPHeaderCallback as PdHTTPHeaderCallback,
    PDNetErr, WifiStatus,
};
use cstr_core::{CStr, CString};

#[derive(Clone, Debug)]
pub struct Network {
    raw_network: *const playdate_network,
    raw_http: *const playdate_http,
    raw_tcp: *const playdate_tcp,
}

static mut NETWORK: Network = Network::null();

type EnableCallback = dyn FnMut(PDNetErr) + 'static;
static mut NETWORK_ENABLE_CALLBACK: Option<Box<EnableCallback>> = None;

extern "C" fn wifi_enable_callback(err: PDNetErr) {
    unsafe {
        if let Some(mut callback) = NETWORK_ENABLE_CALLBACK.take() {
            callback(err);
        }
    }
}

impl Network {
    const fn null() -> Self {
        Self {
            raw_network: ptr::null(),
            raw_http: ptr::null(),
            raw_tcp: ptr::null(),
        }
    }

    pub(crate) fn new(raw_network: *const playdate_network) -> Result<()> {
        ensure!(
            !raw_network.is_null(),
            "Null pointer passed to Network::new"
        );
        let raw_http = unsafe { (*raw_network).http };
        ensure!(!raw_http.is_null(), "Null pointer for HTTP subsystem");
        let raw_tcp = unsafe { (*raw_network).tcp };
        ensure!(!raw_tcp.is_null(), "Null pointer for TCP subsystem");
        let network = Self {
            raw_network,
            raw_http,
            raw_tcp,
        };
        unsafe { NETWORK = network };
        Ok(())
    }

    pub fn get() -> Self {
        unsafe { NETWORK.clone() }
    }

    fn api(&self) -> &playdate_network {
        unsafe { &*self.raw_network }
    }

    pub fn http(&self) -> Http {
        Http {
            raw_http: self.raw_http,
        }
    }

    fn http_api_ref() -> Option<&'static playdate_http> {
        unsafe { NETWORK.raw_http.as_ref() }
    }

    pub fn status(&self) -> Result<WifiStatus> {
        pd_func_caller!(self.api().getStatus)
    }

    pub fn set_enabled(&self, flag: bool) -> Result<()> {
        self.set_enabled_internal(flag, None)
    }

    pub fn set_enabled_with_callback<F>(&self, flag: bool, callback: F) -> Result<()>
    where
        F: FnMut(PDNetErr) + 'static,
    {
        ensure!(flag, "Callback is only supported when enabling Wi-Fi");
        unsafe {
            ensure!(
                NETWORK_ENABLE_CALLBACK.is_none(),
                "A previous set_enabled_with_callback call is still pending"
            );
            NETWORK_ENABLE_CALLBACK = Some(Box::new(callback));
        }
        match self.set_enabled_internal(true, Some(wifi_enable_callback)) {
            Ok(()) => Ok(()),
            Err(err) => {
                unsafe {
                    NETWORK_ENABLE_CALLBACK = None;
                }
                Err(err)
            }
        }
    }

    fn set_enabled_internal(
        &self,
        flag: bool,
        callback: Option<unsafe extern "C" fn(PDNetErr)>,
    ) -> Result<()> {
        pd_func_caller!(self.api().setEnabled, flag, callback)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Http {
    raw_http: *const playdate_http,
}

impl Http {
    fn api(&self) -> &playdate_http {
        unsafe { &*self.raw_http }
    }

    pub fn request_access<F>(
        &self,
        server: Option<&str>,
        port: i32,
        use_ssl: bool,
        purpose: Option<&str>,
        callback: Option<F>,
    ) -> Result<accessReply>
    where
        F: FnMut(bool) + 'static,
    {
        let server_c = optional_cstring(server)?;
        let purpose_c = optional_cstring(purpose)?;
        let server_ptr = server_c.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null());
        let purpose_ptr = purpose_c
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null());
        let mut callback_userdata = ptr::null_mut();
        let mut callback_state: *mut AccessRequestState = ptr::null_mut();
        let callback_fn = if let Some(cb) = callback {
            let state = Box::new(AccessRequestState {
                callback: Some(Box::new(cb)),
            });
            callback_state = Box::into_raw(state);
            callback_userdata = callback_state as *mut ctypes::c_void;
            Some(http_access_request_callback as unsafe extern "C" fn(bool, *mut ctypes::c_void))
        } else {
            None
        };
        let reply = pd_func_caller!(
            self.api().requestAccess,
            server_ptr,
            port,
            use_ssl,
            purpose_ptr,
            callback_fn,
            callback_userdata
        )?;
        if reply != accessReply::kAccessAsk && !callback_state.is_null() {
            unsafe {
                drop(Box::from_raw(callback_state));
            }
        }
        Ok(reply)
    }

    pub fn new_connection(&self, server: &str, port: i32, use_ssl: bool) -> Result<HttpConnection> {
        ensure!(
            !server.is_empty(),
            "HTTP connections require a non-empty server"
        );
        let server_c = CString::new(server).map_err(Error::msg)?;
        let raw_connection =
            pd_func_caller!(self.api().newConnection, server_c.as_ptr(), port, use_ssl)?;
        ensure!(
            !raw_connection.is_null(),
            "HTTP connection creation returned null (permission denied?)"
        );
        HttpConnection::from_raw(self.raw_http, raw_connection)
    }
}

type AccessRequestClosure = dyn FnMut(bool) + 'static;

struct AccessRequestState {
    callback: Option<Box<AccessRequestClosure>>,
}

extern "C" fn http_access_request_callback(allowed: bool, userdata: *mut ctypes::c_void) {
    if userdata.is_null() {
        return;
    }
    unsafe {
        let mut state: Box<AccessRequestState> = Box::from_raw(userdata as *mut AccessRequestState);
        if let Some(mut callback) = state.callback.take() {
            callback(allowed);
        }
    }
}

#[derive(Default)]
struct HttpCallbackSlots {
    header_received: Option<HeaderCallback>,
    headers_read: Option<SimpleCallback>,
    response: Option<SimpleCallback>,
    request_complete: Option<SimpleCallback>,
    connection_closed: Option<SimpleCallback>,
}

type SimpleCallback = Box<dyn FnMut(&HttpConnection) + 'static>;
type SimpleCallbackPtr = *mut (dyn FnMut(&HttpConnection) + 'static);

type HeaderCallback = Box<dyn FnMut(&HttpConnection, &CStr, &CStr) + 'static>;
type HeaderCallbackPtr = *mut (dyn FnMut(&HttpConnection, &CStr, &CStr) + 'static);

struct HttpConnectionInner {
    raw_http: *const playdate_http,
    raw_connection: *mut HTTPConnection,
    callbacks: RefCell<HttpCallbackSlots>,
}

impl Drop for HttpConnectionInner {
    fn drop(&mut self) {
        fn do_drop(conn: &mut HttpConnectionInner) -> Result<()> {
            unsafe {
                let userdata = pd_func_caller!((*conn.raw_http).getUserdata, conn.raw_connection)?;
                // Drop weak count. NB this is OK because we don't race (we're single threaded)
                Weak::from_raw(userdata as *mut ctypes::c_void);
                pd_func_caller!(
                    (*conn.raw_http).setUserdata,
                    conn.raw_connection,
                    ptr::null_mut()
                )?;
                pd_func_caller!((*conn.raw_http).close, conn.raw_connection)?;
                pd_func_caller!((*conn.raw_http).release, conn.raw_connection)?;
            }
            Ok(())
        }
        do_drop(self).unwrap();
    }
}

#[derive(Clone)]
pub struct HttpConnection {
    inner: Rc<HttpConnectionInner>,
}

fn connection_from_userdata(conn: *mut HTTPConnection) -> Option<HttpConnection> {
    let api = Network::http_api_ref()?;
    let get_userdata = api.getUserdata?;
    let userdata = unsafe { get_userdata(conn) };
    if userdata.is_null() {
        return None;
    }
    let inner_ptr = userdata as *const HttpConnectionInner;
    unsafe {
        let weak = ManuallyDrop::new(Weak::from_raw(inner_ptr)); // stop weak count being decremented by this function
        if let Some(inner) = Weak::upgrade(&weak) {
            Some(HttpConnection { inner })
        } else {
            None
        }
    }
}

fn run_simple_callback(
    conn: *mut HTTPConnection,
    accessor: impl Fn(&mut HttpCallbackSlots) -> Option<SimpleCallbackPtr>,
) {
    if let Some(connection) = connection_from_userdata(conn) {
        let mut callbacks = connection.inner.callbacks.borrow_mut();
        let callback_ptr = accessor(&mut callbacks);
        drop(callbacks);
        if let Some(callback_ptr) = callback_ptr {
            unsafe {
                (*callback_ptr)(&connection);
            }
        }
    }
}

fn run_header_callback(
    conn: *mut HTTPConnection,
    key: *const ctypes::c_char,
    value: *const ctypes::c_char,
) {
    if key.is_null() || value.is_null() {
        return;
    }
    if let Some(connection) = connection_from_userdata(conn) {
        let mut callbacks = connection.inner.callbacks.borrow_mut();
        let callback_ptr = callbacks
            .header_received
            .as_mut()
            .map(|cb| &mut **cb as HeaderCallbackPtr);
        drop(callbacks);
        if let Some(callback_ptr) = callback_ptr {
            let key_cstr = unsafe { CStr::from_ptr(key) };
            let value_cstr = unsafe { CStr::from_ptr(value) };
            unsafe {
                (*callback_ptr)(&connection, key_cstr, value_cstr);
            }
        }
    }
}

extern "C" fn http_header_received_trampoline(
    conn: *mut HTTPConnection,
    key: *const ctypes::c_char,
    value: *const ctypes::c_char,
) {
    run_header_callback(conn, key, value);
}

extern "C" fn http_headers_read_trampoline(conn: *mut HTTPConnection) {
    run_simple_callback(conn, |slots| {
        slots
            .headers_read
            .as_mut()
            .map(|cb| &mut **cb as SimpleCallbackPtr)
    });
}

extern "C" fn http_response_trampoline(conn: *mut HTTPConnection) {
    run_simple_callback(conn, |slots| {
        slots
            .response
            .as_mut()
            .map(|cb| &mut **cb as SimpleCallbackPtr)
    });
}

extern "C" fn http_request_complete_trampoline(conn: *mut HTTPConnection) {
    run_simple_callback(conn, |slots| {
        slots
            .request_complete
            .as_mut()
            .map(|cb| &mut **cb as SimpleCallbackPtr)
    });
}

extern "C" fn http_connection_closed_trampoline(conn: *mut HTTPConnection) {
    run_simple_callback(conn, |slots| {
        slots
            .connection_closed
            .as_mut()
            .map(|cb| &mut **cb as SimpleCallbackPtr)
    });
}

impl HttpConnection {
    fn from_raw(
        raw_http: *const playdate_http,
        raw_connection: *mut HTTPConnection,
    ) -> Result<Self> {
        ensure!(
            !raw_http.is_null(),
            "HTTP subsystem pointer must not be null"
        );
        ensure!(
            !raw_connection.is_null(),
            "HTTP connection pointer must not be null"
        );
        let inner = Rc::new(HttpConnectionInner {
            raw_http,
            raw_connection,
            callbacks: RefCell::new(HttpCallbackSlots::default()),
        });
        let userdata_ptr = Weak::into_raw(Rc::downgrade(&inner)) as *mut ctypes::c_void;
        pd_func_caller!((*raw_http).setUserdata, raw_connection, userdata_ptr)?;
        Ok(Self { inner })
    }

    fn api(&self) -> &playdate_http {
        unsafe { &*self.inner.raw_http }
    }

    pub fn raw_connection(&self) -> *mut HTTPConnection {
        self.inner.raw_connection
    }

    pub fn set_connect_timeout(&self, timeout_ms: u32) -> Result<()> {
        pd_func_caller!(
            self.api().setConnectTimeout,
            self.raw_connection(),
            timeout_ms.try_into().map_err(Error::msg)?
        )
    }

    pub fn set_keep_alive(&self, keep_alive: bool) -> Result<()> {
        pd_func_caller!(self.api().setKeepAlive, self.raw_connection(), keep_alive)
    }

    pub fn set_byte_range(&self, start: u32, end: u32) -> Result<()> {
        pd_func_caller!(
            self.api().setByteRange,
            self.raw_connection(),
            start.try_into().map_err(Error::msg)?,
            end.try_into().map_err(Error::msg)?
        )
    }

    pub fn get(&self, path: &str, headers: Option<&[u8]>) -> Result<()> {
        let path_c = CString::new(path).map_err(Error::msg)?;
        let (headers_ptr, header_len) = buffer_ptr_and_len(headers);
        let err = pd_func_caller!(
            self.api().get,
            self.raw_connection(),
            path_c.as_ptr(),
            headers_ptr,
            header_len
        )?;
        ensure_net_ok(err, "http.get")
    }

    pub fn post(&self, path: &str, headers: Option<&[u8]>, body: Option<&[u8]>) -> Result<()> {
        let path_c = CString::new(path).map_err(Error::msg)?;
        let (headers_ptr, header_len) = buffer_ptr_and_len(headers);
        let (body_ptr, body_len) = buffer_ptr_and_len(body);
        let err = pd_func_caller!(
            self.api().post,
            self.raw_connection(),
            path_c.as_ptr(),
            headers_ptr,
            header_len,
            body_ptr,
            body_len
        )?;
        ensure_net_ok(err, "http.post")
    }

    pub fn query(
        &self,
        method: &str,
        path: &str,
        headers: Option<&[u8]>,
        body: Option<&[u8]>,
    ) -> Result<()> {
        let method_c = CString::new(method).map_err(Error::msg)?;
        let path_c = CString::new(path).map_err(Error::msg)?;
        let (headers_ptr, header_len) = buffer_ptr_and_len(headers);
        let (body_ptr, body_len) = buffer_ptr_and_len(body);
        let err = pd_func_caller!(
            self.api().query,
            self.raw_connection(),
            method_c.as_ptr(),
            path_c.as_ptr(),
            headers_ptr,
            header_len,
            body_ptr,
            body_len
        )?;
        ensure_net_ok(err, "http.query")
    }

    pub fn error(&self) -> Result<PDNetErr> {
        pd_func_caller!(self.api().getError, self.raw_connection())
    }

    pub fn progress(&self) -> Result<(i32, i32)> {
        let mut read = 0;
        let mut total = 0;
        pd_func_caller!(
            self.api().getProgress,
            self.raw_connection(),
            &mut read,
            &mut total
        )?;
        Ok((read, total))
    }

    pub fn response_status(&self) -> Result<i32> {
        pd_func_caller!(self.api().getResponseStatus, self.raw_connection())
    }

    pub fn bytes_available(&self) -> Result<usize> {
        pd_func_caller!(self.api().getBytesAvailable, self.raw_connection())
    }

    pub fn set_read_timeout(&self, timeout_ms: u32) -> Result<()> {
        pd_func_caller!(
            self.api().setReadTimeout,
            self.raw_connection(),
            timeout_ms.try_into().map_err(Error::msg)?
        )
    }

    pub fn set_read_buffer_size(&self, bytes: u32) -> Result<()> {
        pd_func_caller!(
            self.api().setReadBufferSize,
            self.raw_connection(),
            bytes.try_into().map_err(Error::msg)?
        )
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize> {
        assert!(
            !buffer.is_empty(),
            "Buffer must not be empty to distinguish from EOF"
        );
        let len = len_to_c_uint(buffer.len())?;
        let result = pd_func_caller!(
            self.api().read,
            self.raw_connection(),
            buffer.as_mut_ptr() as *mut ctypes::c_void,
            len
        )?;
        if result >= 0 {
            Ok(result as usize)
        } else {
            Err(anyhow!(
                "http.read returned error {}",
                describe_net_err(result)
            ))
        }
    }

    pub fn discard(&self, len: usize) -> Result<usize> {
        if len == 0 {
            return Ok(0);
        }
        let len = len_to_c_uint(len)?;
        let result = pd_func_caller!(self.api().read, self.raw_connection(), ptr::null_mut(), len)?;
        if result >= 0 {
            Ok(result as usize)
        } else {
            Err(anyhow!(
                "http.read(discard) returned error {}",
                describe_net_err(result)
            ))
        }
    }

    pub fn close(&self) {
        unsafe {
            if let Some(close) = self.api().close {
                close(self.raw_connection());
            }
        }
    }

    pub fn on_header_received<F>(&self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(&HttpConnection, &CStr, &CStr) + 'static,
    {
        let mut slots = self.inner.callbacks.borrow_mut();
        slots.header_received = callback.map(|cb| Box::new(cb) as HeaderCallback);
        let register = slots.header_received.is_some();
        drop(slots);
        let trampoline: PdHTTPHeaderCallback = if register {
            Some(http_header_received_trampoline)
        } else {
            None
        };
        pd_func_caller!(
            self.api().setHeaderReceivedCallback,
            self.raw_connection(),
            trampoline
        )
    }

    pub fn on_headers_read<F>(&self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(&HttpConnection) + 'static,
    {
        self.configure_simple_callback(
            callback,
            |slots| &mut slots.headers_read,
            http_headers_read_trampoline,
            self.api().setHeadersReadCallback,
        )
    }

    pub fn on_response<F>(&self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(&HttpConnection) + 'static,
    {
        self.configure_simple_callback(
            callback,
            |slots| &mut slots.response,
            http_response_trampoline,
            self.api().setResponseCallback,
        )
    }

    pub fn on_request_complete<F>(&self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(&HttpConnection) + 'static,
    {
        self.configure_simple_callback(
            callback,
            |slots| &mut slots.request_complete,
            http_request_complete_trampoline,
            self.api().setRequestCompleteCallback,
        )
    }

    pub fn on_connection_closed<F>(&self, callback: Option<F>) -> Result<()>
    where
        F: FnMut(&HttpConnection) + 'static,
    {
        self.configure_simple_callback(
            callback,
            |slots| &mut slots.connection_closed,
            http_connection_closed_trampoline,
            self.api().setConnectionClosedCallback,
        )
    }

    fn configure_simple_callback<F>(
        &self,
        callback: Option<F>,
        slot: impl Fn(&mut HttpCallbackSlots) -> &mut Option<SimpleCallback>,
        trampoline: unsafe extern "C" fn(*mut HTTPConnection),
        setter: Option<unsafe extern "C" fn(*mut HTTPConnection, PdHTTPConnectionCallback)>,
    ) -> Result<()>
    where
        F: FnMut(&HttpConnection) + 'static,
    {
        let mut slots = self.inner.callbacks.borrow_mut();
        let slot_ref = slot(&mut slots);
        *slot_ref = callback.map(|cb| Box::new(cb) as SimpleCallback);
        let register = slot_ref.is_some();
        drop(slots);
        let callback_fn = setter.ok_or_else(|| {
            anyhow!(
                "HTTP subsystem does not expose the requested callback: {:?}",
                self.inner.raw_http
            )
        })?;
        let fn_ptr: PdHTTPConnectionCallback = if register { Some(trampoline) } else { None };
        unsafe {
            callback_fn(self.raw_connection(), fn_ptr);
        }
        Ok(())
    }
}

fn optional_cstring(value: Option<&str>) -> Result<Option<CString>> {
    value
        .map(|s| CString::new(s).map_err(Error::msg))
        .transpose()
}

fn buffer_ptr_and_len(buffer: Option<&[u8]>) -> (*const ctypes::c_char, usize) {
    match buffer {
        Some(data) if !data.is_empty() => (data.as_ptr() as *const ctypes::c_char, data.len()),
        _ => (ptr::null(), 0),
    }
}

fn ensure_net_ok(err: PDNetErr, context: &str) -> Result<()> {
    if matches!(err, PDNetErr::NET_OK) {
        Ok(())
    } else {
        Err(anyhow!("{context} failed with {:?}", err))
    }
}

fn describe_net_err(value: i32) -> String {
    match value {
        x if x == PDNetErr::NET_OK as i32 => "NET_OK".into(),
        x if x == PDNetErr::NET_NO_DEVICE as i32 => "NET_NO_DEVICE".into(),
        x if x == PDNetErr::NET_BUSY as i32 => "NET_BUSY".into(),
        x if x == PDNetErr::NET_WRITE_ERROR as i32 => "NET_WRITE_ERROR".into(),
        x if x == PDNetErr::NET_WRITE_BUSY as i32 => "NET_WRITE_BUSY".into(),
        x if x == PDNetErr::NET_WRITE_TIMEOUT as i32 => "NET_WRITE_TIMEOUT".into(),
        x if x == PDNetErr::NET_READ_ERROR as i32 => "NET_READ_ERROR".into(),
        x if x == PDNetErr::NET_READ_BUSY as i32 => "NET_READ_BUSY".into(),
        x if x == PDNetErr::NET_READ_TIMEOUT as i32 => "NET_READ_TIMEOUT".into(),
        x if x == PDNetErr::NET_READ_OVERFLOW as i32 => "NET_READ_OVERFLOW".into(),
        x if x == PDNetErr::NET_FRAME_ERROR as i32 => "NET_FRAME_ERROR".into(),
        x if x == PDNetErr::NET_BAD_RESPONSE as i32 => "NET_BAD_RESPONSE".into(),
        x if x == PDNetErr::NET_ERROR_RESPONSE as i32 => "NET_ERROR_RESPONSE".into(),
        x if x == PDNetErr::NET_RESET_TIMEOUT as i32 => "NET_RESET_TIMEOUT".into(),
        x if x == PDNetErr::NET_BUFFER_TOO_SMALL as i32 => "NET_BUFFER_TOO_SMALL".into(),
        x if x == PDNetErr::NET_UNEXPECTED_RESPONSE as i32 => "NET_UNEXPECTED_RESPONSE".into(),
        x if x == PDNetErr::NET_NOT_CONNECTED_TO_AP as i32 => "NET_NOT_CONNECTED_TO_AP".into(),
        x if x == PDNetErr::NET_NOT_IMPLEMENTED as i32 => "NET_NOT_IMPLEMENTED".into(),
        x if x == PDNetErr::NET_CONNECTION_CLOSED as i32 => "NET_CONNECTION_CLOSED".into(),
        other => format!("Unknown({other})"),
    }
}

fn len_to_c_uint(len: usize) -> Result<ctypes::c_uint> {
    if len > u32::MAX as usize {
        Err(anyhow!("Length {} exceeds c_uint max", len))
    } else {
        Ok(len as ctypes::c_uint)
    }
}
