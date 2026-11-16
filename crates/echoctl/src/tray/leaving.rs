//! The tray: leaving — quit, restart and update, asked and done in steps (split out of `tray.rs`, moved as it was).

use super::*;

/// Where the leave stands: one at a time, from the window's save handshake to Echo stopping. A
/// leave asked meanwhile takes the pending one's place (`takes_over`) or is dropped — while the
/// user is asked, it waits for the answer instead (`queued`), so a question is never answered for
/// another leave; once Echo is stopping, every one is dropped. The event loop does what each step
/// says (`act`).
#[derive(Default)]
pub(super) struct Leaving {
    phase: Phase,
    handshakes: u64,
    /// Asked while the user is asked about another leave: started after that answer (it saves and
    /// asks its own question).
    queued: Option<Then>,
    /// `quit.asking` is written (`asking_quit`).
    asking_file: bool,
}

#[derive(Debug, Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    /// The window's page saves the notes it holds (`prepareClose`, handshake `n`; 5 s at most).
    Saving(Then, u64),
    /// The save failed or the page didn't answer: the user is asked whether to go on anyway.
    Asking(Then),
    /// A quit, an update or a folder change went ahead: Echo is stopping.
    Going,
}

/// What the event loop does for the leave (`act`).
#[derive(Debug)]
pub(super) enum Step {
    Nothing,
    /// The page saves its notes (`prepareClose(n, close)`; a close explains a failure itself).
    Save { n: u64, close: bool },
    /// Do it (`go`); then `went` says whether Echo stops.
    Go(Then),
    /// Ask the user whether to go on anyway (`ask_to_leave`).
    Ask(String),
    /// It didn't happen: the editors writable again, and whoever waits on a quit told why.
    Stay(Then, String),
    /// Close the window only: the leave still waits for the user's answer.
    CloseWindow,
}

impl Leaving {
    /// A leave is asked for; `window`: a page holds notes to save first.
    pub(super) fn start(&mut self, then: Then, window: bool) -> Step {
        match &mut self.phase {
            Phase::Going => Step::Nothing,
            Phase::Saving(pending, _) => {
                if takes_over(pending, &then) {
                    *pending = then;
                }
                Step::Nothing
            }
            Phase::Asking(pending) => {
                if takes_over(pending, &then) && self.queued.as_ref().is_none_or(|q| takes_over(q, &then)) {
                    self.queued = Some(then);
                }
                Step::Nothing
            }
            Phase::Idle if !window => Step::Go(then),
            Phase::Idle => {
                self.handshakes += 1;
                let close = matches!(then, Then::CloseWindow);
                self.phase = Phase::Saving(then, self.handshakes);
                Step::Save { n: self.handshakes, close }
            }
        }
    }

    /// The page answered handshake `n` (`ok`: its notes are saved), or didn't in time (None). An
    /// answer or a timeout for another handshake changes nothing.
    pub(super) fn answered(&mut self, n: u64, ok: Option<bool>) -> Step {
        if !matches!(self.phase, Phase::Saving(_, m) if m == n) {
            return Step::Nothing;
        }
        let Phase::Saving(then, _) = std::mem::take(&mut self.phase) else { return Step::Nothing };
        match after_handshake(then, ok) {
            Next::Go(t) => Step::Go(t),
            Next::Stay => Step::Nothing,
            Next::Ask(t, why) => {
                let question = format!("{why}\n\n{}", t.question());
                self.phase = Phase::Asking(t);
                Step::Ask(question)
            }
        }
    }

    /// The user's answer to `Ask` (Err: they couldn't be asked).
    pub(super) fn decided(&mut self, answer: Result<bool, String>) -> Step {
        match (std::mem::take(&mut self.phase), answer) {
            (Phase::Asking(then), Ok(true)) => Step::Go(then),
            (Phase::Asking(then), Ok(false)) => Step::Stay(then, "notes typed in its window couldn't be saved, and Echo was kept open".into()),
            (Phase::Asking(then), Err(why)) => {
                Step::Stay(then, format!("notes typed in its window couldn't be saved, and Echo couldn't ask whether to go on ({why})"))
            }
            (other, _) => {
                self.phase = other;
                Step::Nothing
            }
        }
    }

    /// The page's Close without saving (after a close it couldn't save): a leave still saving goes
    /// on without the notes; one waiting for the user's answer keeps waiting, the window closes.
    pub(super) fn close_now(&mut self) -> Step {
        match std::mem::take(&mut self.phase) {
            Phase::Idle => Step::Go(Then::CloseWindow),
            Phase::Saving(then, _) => Step::Go(then),
            asking @ Phase::Asking(_) => {
                self.phase = asking;
                Step::CloseWindow
            }
            Phase::Going => {
                self.phase = Phase::Going;
                Step::Nothing
            }
        }
    }

    /// What `go` did: Echo stops, or it stays (a window closed; an update or a folder change that
    /// failed).
    pub(super) fn went(&mut self, stopping: bool) {
        self.phase = if stopping { Phase::Going } else { Phase::Idle };
    }

    /// A leave that waited for the user's answer about another, once that one is done.
    pub(super) fn next(&mut self) -> Option<Then> {
        if matches!(self.phase, Phase::Asking(_)) { None } else { self.queued.take() }
    }

    /// The quit `echoctl app quit` waits for is held up by a question to the user (`quit.asking`
    /// while so: the CLI keeps waiting).
    /// Echo is stopping, or the user is being asked whether it may (a quit, an update, a folder change):
    /// no window opens now — one opened then would lose what is typed in it.
    pub(super) fn may_open(&self) -> bool {
        !matches!(self.phase, Phase::Going | Phase::Asking(Then::Quit | Then::QuitAsked | Then::Update(_) | Then::ChangeHome(_)))
    }

    pub(super) fn asking_quit(&self) -> bool {
        matches!(self.phase, Phase::Asking(Then::QuitAsked)) || matches!(self.queued, Some(Then::QuitAsked))
    }
}

/// A leave asked while another is pending (saving, or asking the user): does it take that one's
/// place? Anything replaces a close; a quit replaces an update or a folder change (what was asked
/// last wins); `echoctl app quit` replaces a quit (it waits for the outcome). Anything else is
/// dropped: what is pending stops Echo too. So macOS's own quit, which waits for an answer, always
/// rides on a quit that ends in `go` (answered yes on exit) or `Stay` (answered no).
pub(super) fn takes_over(pending: &Then, new: &Then) -> bool {
    matches!(
        (pending, new),
        (Then::CloseWindow, _) | (Then::Update(_) | Then::ChangeHome(_), Then::Quit | Then::QuitAsked) | (Then::Quit, Then::QuitAsked)
    )
}

/// What follows the window's answer to the save handshake (or its silence).
#[derive(Debug)]
pub(super) enum Next {
    Go(Then),
    Ask(Then, &'static str),
    /// A close the page explains itself (Try again / Close without saving).
    Stay,
}

/// `close_ready`: saved → go on; not saved → the page says so for a close, the user is asked
/// for anything else. No answer (`ok` None) → the user is asked, a close too.
pub(super) fn after_handshake(then: Then, ok: Option<bool>) -> Next {
    match (ok, then) {
        (Some(true), t) => Next::Go(t),
        (Some(false), Then::CloseWindow) => Next::Stay,
        (Some(false), t) => Next::Ask(t, "Notes typed in the Echo window couldn't be saved."),
        (None, t) => Next::Ask(t, "The Echo window didn't confirm that the notes typed in it are saved."),
    }
}

/// Ask whether to leave anyway, off the event loop (`LeaveDecided`: once, also if the dialog's
/// thread fails — the leave would wait for ever).
pub(super) fn ask_to_leave(question: String, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    struct Answer(Option<tao::event_loop::EventLoopProxy<UiEvent>>);
    impl Answer {
        fn send(&mut self, answer: Result<bool, String>) {
            if let Some(p) = self.0.take() {
                let _ = p.send_event(UiEvent::LeaveDecided(answer));
            }
        }
    }
    impl Drop for Answer {
        fn drop(&mut self) {
            self.send(Err("the question ended without an answer".into()));
        }
    }
    let mut answer = Answer(Some(proxy.clone()));
    std::thread::spawn(move || {
        let a = app::ask_checked(&question, &["Cancel", "Go on"], "Cancel").map(|a| a.as_deref() == Some("Go on")).map_err(|e| format!("{e:#}"));
        answer.send(a);
    });
}

/// Do what the leave's step says.
pub(super) fn act(step: Step, l: &mut Leaving, win: &mut Option<crate::window::EchoWindow>, stop: &Arc<AtomicBool>, ui: &mut Option<Ui>, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    match step {
        Step::Nothing => {}
        Step::Save { n, close } => {
            if let Some(w) = win {
                let _ = w.webview.evaluate_script(&format!(
                    "window.echo && window.echo.prepareClose ? window.echo.prepareClose({n}, {close}) : window.ipc.postMessage(JSON.stringify({{ cmd: 'close_ready', ok: true, n: {n} }}))"
                ));
            }
            let p = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(5));
                let _ = p.send_event(UiEvent::LeaveTimeout(n));
            });
        }
        Step::Go(then) => l.went(go(then, win, stop, ui)),
        Step::Ask(question) => ask_to_leave(question, proxy),
        Step::Stay(then, why) => {
            thaw(win);
            match then {
                Then::QuitAsked => app::quit_cancelled(&why),
                Then::Update(_) => app::notify("Update not installed", "Notes typed in the Echo window couldn't be saved. Install it from the menu once they are."),
                Then::ChangeHome(_) => app::notify("Data folder not changed", "Notes typed in the Echo window couldn't be saved."),
                Then::CloseWindow | Then::Quit => {}
            }
            #[cfg(target_os = "macos")]
            if matches!(then, Then::Quit | Then::QuitAsked) {
                terminate::cancelled();
            }
        }
        Step::CloseWindow => {
            *win = None;
            apply_dock(false);
        }
    }
    if l.asking_quit() != l.asking_file {
        l.asking_file = l.asking_quit();
        app::asking_to_quit(l.asking_file);
    }
    // A leave that waited for that answer: its turn now.
    if let Some(next) = l.next() {
        let step = l.start(next, win.is_some());
        act(step, l, win, stop, ui, proxy);
    }
}
