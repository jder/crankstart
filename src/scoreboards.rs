use crate::pd_func_caller;
use alloc::{
    collections::VecDeque,
    format,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::{ensure, Error, Result};
use core::ptr;
use crankstart_sys::{
    ctypes, playdate_scoreboards, PDBoard, PDBoardsList, PDListScore, PDScore, PDScoresList,
};
use cstr_core::{CStr, CString};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreboardOperation {
    AddScore,
    GetPersonalBest,
    GetScoreboards,
    GetScores,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreboardError {
    pub operation: ScoreboardOperation,
    pub board_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Score {
    pub rank: u32,
    pub value: u32,
    pub player: String,
    pub board_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreEntry {
    pub rank: u32,
    pub value: u32,
    pub player: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoresList {
    pub board_id: String,
    pub last_updated: u32,
    pub player_included: bool,
    pub limit: u32,
    pub scores: Vec<ScoreEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Board {
    pub board_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardsList {
    pub last_updated: u32,
    pub boards: Vec<Board>,
}

pub(crate) enum ScoreboardEvent {
    ScoreAdded(core::result::Result<Score, ScoreboardError>),
    PersonalBest(core::result::Result<Option<Score>, ScoreboardError>),
    Scoreboards(core::result::Result<BoardsList, ScoreboardError>),
    Scores(core::result::Result<ScoresList, ScoreboardError>),
}

#[derive(Clone, Debug)]
pub struct Scoreboards(*const playdate_scoreboards);

static mut SCOREBOARDS: Scoreboards = Scoreboards(ptr::null());
static mut SCOREBOARD_EVENTS: VecDeque<ScoreboardEvent> = const { VecDeque::new() };

impl Scoreboards {
    pub(crate) fn new(raw_scoreboards: *const playdate_scoreboards) -> Result<()> {
        ensure!(
            !raw_scoreboards.is_null(),
            "Null pointer passed to Scoreboards::new"
        );
        unsafe {
            SCOREBOARDS = Self(raw_scoreboards);
        }
        Ok(())
    }

    pub fn get() -> Self {
        unsafe { (&raw const SCOREBOARDS).read().clone() }
    }

    fn api(&self) -> &playdate_scoreboards {
        unsafe { &*self.0 }
    }

    /// Starts adding a score. Returns an error if the SDK rejects the request;
    /// the result is later delivered to `Game::score_added`.
    pub fn add_score(&self, board_id: &str, value: u32) -> Result<()> {
        let board_id = board_id_string(board_id)?;
        ensure_request_accepted(pd_func_caller!(
            self.api().addScore,
            board_id.as_ptr(),
            value,
            Some(add_score_callback)
        ))
    }

    /// Starts fetching a personal best. The result is later delivered to
    /// `Game::personal_best_received`. Returns an error if the SDK rejects the request.
    pub fn get_personal_best(&self, board_id: &str) -> Result<()> {
        let board_id = board_id_string(board_id)?;
        ensure_request_accepted(pd_func_caller!(
            self.api().getPersonalBest,
            board_id.as_ptr(),
            Some(personal_best_callback)
        ))
    }

    /// Starts fetching the game's boards. The result is later delivered to
    /// `Game::scoreboards_received`. Returns an error if the SDK rejects the request.
    pub fn get_scoreboards(&self) -> Result<()> {
        ensure_request_accepted(pd_func_caller!(
            self.api().getScoreboards,
            Some(scoreboards_callback)
        ))
    }

    /// Starts fetching a board's scores. The result is later delivered to
    /// `Game::scores_received`. Returns an error if the SDK rejects the request.
    pub fn get_scores(&self, board_id: &str) -> Result<()> {
        let board_id = board_id_string(board_id)?;
        ensure_request_accepted(pd_func_caller!(
            self.api().getScores,
            board_id.as_ptr(),
            Some(scores_callback)
        ))
    }

    pub(crate) fn pop_event() -> Option<ScoreboardEvent> {
        let events = unsafe { &mut *(&raw mut SCOREBOARD_EVENTS) };
        events.pop_front()
    }

    fn push_event(event: ScoreboardEvent) {
        let events = unsafe { &mut *(&raw mut SCOREBOARD_EVENTS) };
        events.push_back(event);
    }

    fn free_score(&self, score: *mut PDScore) -> Result<()> {
        pd_func_caller!(self.api().freeScore, score)
    }

    fn free_boards_list(&self, boards: *mut PDBoardsList) -> Result<()> {
        pd_func_caller!(self.api().freeBoardsList, boards)
    }

    fn free_scores_list(&self, scores: *mut PDScoresList) -> Result<()> {
        pd_func_caller!(self.api().freeScoresList, scores)
    }
}

fn ensure_request_accepted(result: Result<i32>) -> Result<()> {
    let status = result?;
    ensure!(
        status == 1,
        "Scoreboard request rejected with status {status}"
    );
    Ok(())
}

fn board_id_string(board_id: &str) -> Result<CString> {
    ensure!(!board_id.is_empty(), "Scoreboard ID must not be empty");
    CString::new(board_id).map_err(Error::msg)
}

unsafe fn copy_string(value: *const ctypes::c_char, field: &str) -> Result<String> {
    ensure!(
        !value.is_null(),
        "Scoreboard callback returned null {field}"
    );
    Ok(unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned())
}

unsafe fn copy_optional_string(value: *const ctypes::c_char) -> Option<String> {
    (!value.is_null()).then(|| {
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    })
}

unsafe fn copy_score(score: &PDScore) -> Result<Score> {
    Ok(Score {
        rank: score.rank,
        value: score.value,
        player: unsafe { copy_string(score.player, "score player") }?,
        board_id: unsafe { copy_string(score.boardID, "score board ID") }?,
    })
}

unsafe fn copy_score_entry(score: &PDListScore) -> Result<ScoreEntry> {
    Ok(ScoreEntry {
        rank: score.rank,
        value: score.value,
        player: unsafe { copy_string(score.player, "score player") }?,
    })
}

unsafe fn copy_scores_list(scores: &PDScoresList) -> Result<ScoresList> {
    ensure!(
        scores.count == 0 || !scores.scores.is_null(),
        "Scoreboard callback returned a null scores array with a nonzero count"
    );
    let raw_scores = if scores.count == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(scores.scores, scores.count as usize) }
    };
    let entries = raw_scores
        .iter()
        .map(|score| unsafe { copy_score_entry(score) })
        .collect::<Result<Vec<_>>>()?;
    Ok(ScoresList {
        board_id: unsafe { copy_string(scores.boardID, "scores board ID") }?,
        last_updated: scores.lastUpdated,
        player_included: scores.playerIncluded != 0,
        limit: scores.limit,
        scores: entries,
    })
}

unsafe fn copy_board(board: &PDBoard) -> Result<Board> {
    Ok(Board {
        board_id: unsafe { copy_string(board.boardID, "board ID") }?,
        name: unsafe { copy_string(board.name, "board name") }?,
    })
}

unsafe fn copy_boards_list(boards: &PDBoardsList) -> Result<BoardsList> {
    ensure!(
        boards.count == 0 || !boards.boards.is_null(),
        "Scoreboard callback returned a null boards array with a nonzero count"
    );
    let raw_boards = if boards.count == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(boards.boards, boards.count as usize) }
    };
    let last_updated = boards.lastUpdated;
    let boards = raw_boards
        .iter()
        .map(|board| unsafe { copy_board(board) })
        .collect::<Result<Vec<_>>>()?;
    Ok(BoardsList {
        last_updated,
        boards,
    })
}

fn callback_error(
    operation: ScoreboardOperation,
    error_message: *const ctypes::c_char,
    board_id: Option<String>,
) -> ScoreboardError {
    let message = unsafe { copy_optional_string(error_message) }.unwrap_or_else(|| {
        "Scoreboard callback returned neither a result nor an error".to_string()
    });
    ScoreboardError {
        operation,
        board_id,
        message,
    }
}

fn conversion_error(
    operation: ScoreboardOperation,
    board_id: Option<String>,
    error: Error,
) -> ScoreboardError {
    ScoreboardError {
        operation,
        board_id,
        message: format!("{error:#}"),
    }
}

extern "C" fn add_score_callback(raw_score: *mut PDScore, error_message: *const ctypes::c_char) {
    let scoreboards = Scoreboards::get();
    let board_id = if raw_score.is_null() {
        None
    } else {
        unsafe { copy_optional_string((*raw_score).boardID) }
    };
    let result = if !error_message.is_null() || raw_score.is_null() {
        Err(callback_error(
            ScoreboardOperation::AddScore,
            error_message,
            board_id,
        ))
    } else {
        unsafe { copy_score(&*raw_score) }
            .map_err(|error| conversion_error(ScoreboardOperation::AddScore, board_id, error))
    };
    if !raw_score.is_null() {
        let _ = scoreboards.free_score(raw_score);
    }
    Scoreboards::push_event(ScoreboardEvent::ScoreAdded(result));
}

extern "C" fn personal_best_callback(
    raw_score: *mut PDScore,
    error_message: *const ctypes::c_char,
) {
    let scoreboards = Scoreboards::get();
    let board_id = if raw_score.is_null() {
        None
    } else {
        unsafe { copy_optional_string((*raw_score).boardID) }
    };
    let result = if !error_message.is_null() {
        Err(callback_error(
            ScoreboardOperation::GetPersonalBest,
            error_message,
            board_id,
        ))
    } else if raw_score.is_null() {
        Ok(None)
    } else {
        unsafe { copy_score(&*raw_score) }
            .map(Some)
            .map_err(|error| {
                conversion_error(ScoreboardOperation::GetPersonalBest, board_id, error)
            })
    };
    if !raw_score.is_null() {
        let _ = scoreboards.free_score(raw_score);
    }
    Scoreboards::push_event(ScoreboardEvent::PersonalBest(result));
}

extern "C" fn scoreboards_callback(
    raw_boards: *mut PDBoardsList,
    error_message: *const ctypes::c_char,
) {
    let scoreboards = Scoreboards::get();
    let result = if !error_message.is_null() || raw_boards.is_null() {
        Err(callback_error(
            ScoreboardOperation::GetScoreboards,
            error_message,
            None,
        ))
    } else {
        unsafe { copy_boards_list(&*raw_boards) }
            .map_err(|error| conversion_error(ScoreboardOperation::GetScoreboards, None, error))
    };
    if !raw_boards.is_null() {
        let _ = scoreboards.free_boards_list(raw_boards);
    }
    Scoreboards::push_event(ScoreboardEvent::Scoreboards(result));
}

extern "C" fn scores_callback(raw_scores: *mut PDScoresList, error_message: *const ctypes::c_char) {
    let scoreboards = Scoreboards::get();
    let board_id = if raw_scores.is_null() {
        None
    } else {
        unsafe { copy_optional_string((*raw_scores).boardID) }
    };
    let result = if !error_message.is_null() || raw_scores.is_null() {
        Err(callback_error(
            ScoreboardOperation::GetScores,
            error_message,
            board_id,
        ))
    } else {
        unsafe { copy_scores_list(&*raw_scores) }
            .map_err(|error| conversion_error(ScoreboardOperation::GetScores, board_id, error))
    };
    if !raw_scores.is_null() {
        let _ = scoreboards.free_scores_list(raw_scores);
    }
    Scoreboards::push_event(ScoreboardEvent::Scores(result));
}
