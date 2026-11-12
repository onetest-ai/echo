//! The tray: its menu, built and acted on (split out of `tray.rs`, moved as it was).

use super::*;

impl Ui {
    pub(super) fn new(proxy: tao::event_loop::EventLoopProxy<UiEvent>) -> tray_icon::Result<Self> {
        let (icon, _) = icon_for(&UiState::Idle { in_call: None });
        let builder = TrayIconBuilder::new().with_tooltip("Echo");
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let tray = builder.build()?;
        let mut ui = Ui {
            proxy,
            quit: MenuItem::new("", true, None),
            tray,
            status: MenuItem::new("", false, None),
            record: Submenu::new("", true),
            memo: MenuItem::new("", false, None),
            stop: MenuItem::new("", false, None),
            note: MenuItem::new("", false, None),
            actions: HashMap::new(),
            state: None,
            dark: taskbar_dark(),
            model: None,
            health: health(),
            last_refresh: Instant::now(),
            refresh_owed: false,
            open_window: None,
        };
        ui.rebuild(load_model(&ui.health));
        Ok(ui)
    }

    pub(super) fn item(&mut self, text: &str, enabled: bool, action: Option<Action>) -> MenuItem {
        let it = MenuItem::new(text, enabled, None);
        if let Some(a) = action {
            self.actions.insert(it.id().clone(), a);
        }
        it
    }

    pub(super) fn check(&mut self, text: &str, checked: bool, action: Action) -> CheckMenuItem {
        let it = CheckMenuItem::new(text, true, checked, None);
        self.actions.insert(it.id().clone(), action);
        it
    }

    /// Rebuild the whole menu from `model` (the live status/record/stop items are recreated and
    /// then refreshed from the current state).
    pub(super) fn rebuild(&mut self, model: Model) {
        self.actions.clear();
        let menu = Menu::new();
        self.status = MenuItem::new("", false, None);
        // Record ▸ in the default language (Settings' first) / in another one
        self.record = Submenu::new("Record this call", true);
        let default = model.languages.first().cloned().unwrap_or_else(|| "en".into());
        let first = self.item(&format!("In {}", language_name(&default)), true, Some(Action::Record));
        let _ = self.record.append_items(&[&first, &PredefinedMenuItem::separator()]);
        for code in model.languages.iter().skip(1) {
            let it = self.item(&format!("In {}", language_name(code)), true, Some(Action::RecordIn(code.clone())));
            let _ = self.record.append(&it);
        }
        self.memo = self.item("Record a memo", true, Some(Action::Memo));
        self.stop = self.item("Stop recording", false, Some(Action::Stop));
        self.note = self.item("Add a note…", false, Some(Action::AddNote));
        let sep = PredefinedMenuItem::separator;
        let open = self.item("Open Echo…", true, Some(Action::OpenWindow(None)));
        let _ = menu.append_items(&[&self.status, &sep(), &self.record, &self.memo, &self.stop, &self.note, &sep(), &open, &sep()]);

        // Upcoming meetings (today)
        let _ = menu.append(&MenuItem::new("Upcoming", false, None));
        if model.upcoming.is_empty() {
            let _ = menu.append(&MenuItem::new("   No more meetings today", false, None));
        }
        for m in &model.upcoming {
            let title = format!("{}  {}", local_hm(m), ellipsize(m.subject.as_deref().unwrap_or("Meeting"), 40));
            let sub = Submenu::new(title, true);
            let last = m.subject.as_deref().map(crate::calls::recap).unwrap_or_default();
            if !last.is_empty() {
                let _ = sub.append(&MenuItem::new("Last time:", false, None));
                for b in &last {
                    let _ = sub.append(&MenuItem::new(format!("   • {b}"), false, None));
                }
                let _ = sub.append(&sep());
            }
            let arm = self.item("Record when it starts", true, Some(Action::ArmMeeting(Box::new(m.clone()))));
            let _ = sub.append(&arm);
            if m.join_url.is_some() {
                let join = self.item("Join and record", true, Some(Action::JoinMeeting(Box::new(m.clone()))));
                let _ = sub.append(&join);
            }
            let _ = menu.append(&sub);
        }
        let _ = menu.append(&sep());

        // Recent calls
        let _ = menu.append(&MenuItem::new("Recent calls", false, None));
        if model.recent.is_empty() {
            let _ = menu.append(&MenuItem::new("   No recordings yet", false, None));
        }
        for (label, name) in &model.recent {
            let it = self.item(&format!("   {label}"), true, Some(Action::OpenWindow(Some(name.clone()))));
            let _ = menu.append(&it);
        }
        let folder = self.item("Open calls folder", true, Some(Action::Open(paths::calls_dir())));
        let _ = menu.append_items(&[&folder, &sep()]);

        // Settings
        let settings = Submenu::new("Settings", true);
        let _ = settings.append(&MenuItem::new("Calendar meetings", false, None));
        let s_ask = self.check("   Ask before they start", !model.scheduled_manual, Action::SetScheduled("ask"));
        let s_manual = self.check("   Only when I click Record", model.scheduled_manual, Action::SetScheduled("manual"));
        let _ = settings.append_items(&[&s_ask, &s_manual, &sep()]);
        let _ = settings.append(&MenuItem::new("Calls not on the calendar", false, None));
        let ask = self.check("   Ask when they start", !model.unscheduled_manual, Action::SetUnscheduled("ask"));
        let manual = self.check("   Only when I click Record", model.unscheduled_manual, Action::SetUnscheduled("manual"));
        let _ = settings.append_items(&[&ask, &manual, &sep()]);
        let keep = self.check("Keep audio after transcription", model.keep_audio, Action::ToggleKeepAudio);
        let frames = self.check("Save snapshots of shared screens", model.capture_frames, Action::ToggleFrames);
        let _ = settings.append(&frames);
        let _ = settings.append_items(&[&keep, &sep()]);
        let langs = if model.languages.is_empty() { "en".to_string() } else { model.languages.join(", ") };
        let _ = settings.append_items(&[&sep(), &MenuItem::new(format!("Languages: {langs}"), false, None)]);
        let profile = self.item("Edit my profile (for minutes)…", true, Some(Action::EditProfile));
        let _ = settings.append(&profile);
        let cfg = self.item("Open settings file", true, Some(Action::Open(paths::state_dir().join("config.json"))));
        let _ = settings.append_items(&[&cfg, &sep()]);
        let _ = settings.append(&MenuItem::new(format!("Data folder: {}", ellipsize(&model.home.display().to_string(), 44)), false, None));
        let open_home = self.item("   Open data folder", true, Some(Action::Open(model.home.clone())));
        let recording = matches!(self.state, Some(UiState::Recording { .. }));
        let change = self.item("   Change data folder…", !recording, Some(Action::ChangeHome));
        let login = self.check("Start Echo at login", model.autostart, Action::ToggleAutostart);
        let _ = settings.append_items(&[&open_home, &change, &sep(), &login]);
        if !model.cli_installed {
            let cli = self.item("Install command-line tool (Echo)…", true, Some(Action::InstallCli));
            let _ = settings.append(&cli);
        }
        // Transcription on another computer (`remote.rs`).
        #[cfg(feature = "transcribe")]
        {
            let _ = settings.append_items(&[&sep(), &MenuItem::new("Transcription server", false, None)]);
            match &model.server {
                Some(label) => {
                    let using = MenuItem::new(format!("   Using {}", ellipsize(label, 44)), false, None);
                    let off = self.item("   Stop using it (transcribe here)", true, Some(Action::DisconnectServer));
                    let change = self.item("   Connect to another server…", true, Some(Action::ConnectServer));
                    let _ = settings.append_items(&[&using, &change, &off]);
                }
                None => {
                    let connect = self.item("   Connect to a transcription server…", true, Some(Action::ConnectServer));
                    let _ = settings.append(&connect);
                }
            }
            let serve = self.check("   Transcribe for other computers", model.serving.is_some(), Action::ToggleServing);
            let _ = settings.append(&serve);
            if model.serving.is_some() {
                let show = self.item("   Show URL and API key…", true, Some(Action::ShowServing));
                let _ = settings.append(&show);
            }
        }

        // Speech models: every catalog entry with Use / Download / Remove.
        let models_menu = Submenu::new("Speech models", true);
        // Whisper models first, then the other engines under their name, then speaker
        // detection (only what this build can run).
        let mut section = models::Engine::Whisper.label();
        for m in models::CATALOG.iter().filter(|m| models::runnable(m)) {
            let this = if m.role == models::Role::Speakers { "Speaker detection" } else { m.engine.label() };
            if this != section {
                section = this;
                let _ = models_menu.append_items(&[&sep(), &MenuItem::new(section, false, None)]);
            }
            let downloaded = model.downloaded_models.contains(&m.name);
            let active = model.active_model.as_deref() == Some(m.name);
            let mark = if active { "✓ " } else if downloaded { "   " } else { "↓ " };
            let sub = Submenu::new(format!("{mark}{} · {} MB", m.name, m.size_mb), true);
            let _ = sub.append(&MenuItem::new(m.summary, false, None));
            if m.name == model.recommended {
                let _ = sub.append(&MenuItem::new("Recommended for this computer", false, None));
            }
            if let (Some(calls), true) = (models::used_for(m.name, model.active_model.as_deref()), downloaded) {
                let _ = sub.append(&MenuItem::new(format!("Also used: for {calls}"), false, None));
            }
            let _ = sub.append(&sep());
            if active {
                let _ = sub.append(&MenuItem::new("In use", false, None));
            } else if downloaded && m.role == models::Role::Speakers {
                let on = MenuItem::new("On: the remote side is split by speaker", false, None);
                let remove = self.item(&format!("Remove ({} MB)…", m.size_mb), true, Some(Action::RemoveModel(m.name)));
                let _ = sub.append_items(&[&on, &remove]);
            } else if downloaded {
                let use_it = self.item("Use this model", true, Some(Action::UseModel(m.name)));
                let remove = self.item(&format!("Remove ({} MB)…", m.size_mb), true, Some(Action::RemoveModel(m.name)));
                let _ = sub.append_items(&[&use_it, &remove]);
            } else {
                let it = match model.downloading {
                    Some((name, pct)) if name == m.name => MenuItem::new(format!("Downloading… {pct}%"), false, None),
                    Some(_) => MenuItem::new("Download (another download is running)", false, None),
                    None => self.item(&format!("Download ({} MB)", m.size_mb), true, Some(Action::DownloadModel(m.name))),
                };
                let _ = sub.append(&it);
            }
            let _ = models_menu.append(&sub);
        }
        if let Some((name, pct)) = model.downloading {
            let _ = models_menu.append_items(&[&sep(), &MenuItem::new(format!("Downloading {name}: {pct}%"), false, None)]);
        }

        // Claude Code: the plugin (status line, skills), where Echo runs it for minutes, and the minutes.
        let claude = Submenu::new(if model.plugin_installed { "Claude Code ✓" } else { "Claude Code" }, true);
        if !model.has_claude {
            let _ = claude.append(&MenuItem::new("Claude Code is not installed", false, None));
        } else if model.plugin_installed {
            let _ = claude.append(&MenuItem::new("Echo plugin installed", false, None));
        } else {
            let connect = self.item("Connect Claude Code (install the plugin)", true, Some(Action::ConnectClaude));
            let _ = claude.append(&connect);
        }
        let _ = claude.append_items(&[&sep(), &MenuItem::new("Echo runs Claude Code for minutes in", false, None)]);
        let any = self.check("   The data folder", model.notes_in.is_none(), Action::NotesAnywhere);
        let _ = claude.append(&any);
        if let Some(dir) = &model.notes_in {
            let only = self.check(&format!("   {}", ellipsize(&dir.display().to_string(), 40)), true, Action::NotesChoose);
            let _ = claude.append(&only);
        }
        let choose = self.item("   Choose a folder…", true, Some(Action::NotesChoose));
        let _ = claude.append(&choose);
        let auto = self.check("Write minutes automatically (15 min after a call)", model.auto_minutes, Action::ToggleAutoMinutes);
        let page = self.check("Write a minutes page (HTML file) too", model.minutes_page, Action::ToggleMinutesPage);
        let _ = claude.append_items(&[&sep(), &auto, &page]);

        // AI for calls: which model (and account) writes live notes and minutes (`ai.rs`).
        let ai = Submenu::new("AI for calls", true);
        match &model.ai {
            None => {
                let _ = ai.append(&MenuItem::new("Can't read the AI settings (ai.json)", false, None));
            }
            Some((live, default, profiles)) => {
                let live_item = self.check("Live notes during calls", *live, Action::ToggleLiveNotes);
                let _ = ai.append_items(&[&live_item, &sep(), &MenuItem::new("New calls use", false, None)]);
                // Each with the model that writes its minutes; the default's can be changed here.
                for (id, name, _, _, _, minutes) in profiles {
                    let it = self.check(&ellipsize(&format!("   {name} — minutes: {minutes}"), 70), id == default, Action::DefaultProfile(id.clone()));
                    let _ = ai.append(&it);
                }
                if let Some((_, name, _, _, _, minutes)) = profiles.iter().find(|p| &p.0 == default) {
                    let it = self.item(&ellipsize(&format!("Minutes model for {name}: {minutes}…"), 70), model.has_claude, Some(Action::NotesModel));
                    let _ = ai.append(&it);
                }
                let accounts: Vec<_> = profiles.iter().filter(|p| p.2 == crate::ai::Kind::Claude).collect();
                if !accounts.is_empty() {
                    let _ = ai.append(&sep());
                }
                for (id, _, _, line, other, _) in accounts {
                    let _ = ai.append(&MenuItem::new(ellipsize(line, 70), false, None));
                    let sign = self.item("   Sign in…", true, Some(Action::SignIn(id.clone())));
                    let _ = ai.append(&sign);
                    if let Some(other) = other {
                        let accept = self.item(&ellipsize(&format!("   Use {other}"), 60), true, Some(Action::AcceptAccount(id.clone(), other.clone())));
                        let _ = ai.append(&accept);
                    }
                }
                let _ = ai.append_items(&[&sep(), &MenuItem::new("Add or remove profiles with `echoctl ai`", false, None)]);
            }
        }

        // Health
        let all_ok = model.health.iter().all(|(_, s, _)| s == "✓");
        let health = Submenu::new(if all_ok { "Health ✓" } else { "Health ⚠" }, true);
        for (name, status, url) in &model.health {
            let action = url.clone().filter(|_| status != "✓").map(Action::OpenUrl);
            let it = self.item(&format!("{status}  {name}"), action.is_some(), action);
            let _ = health.append(&it);
        }
        let askp = self.item("Ask for permissions…", !all_ok, Some(Action::AskPermissions));
        let recheck = self.item("Re-check permissions", true, Some(Action::Recheck));
        let _ = health.append_items(&[&sep(), &askp, &recheck]);
        // What transcription runs on (Windows: the GPU unless its test failed, `gpu.rs`).
        #[cfg(feature = "transcribe")]
        {
            let what = match &model.server {
                Some(label) => format!("server {label}"),
                None => crate::gpu::engine_status(),
            };
            let engine = MenuItem::new(format!("Speech engine: {what}"), false, None);
            let _ = health.append_items(&[&sep(), &engine]);
            if cfg!(windows) {
                let retest = self.item("Re-test GPU", true, Some(Action::RetestGpu));
                let _ = health.append(&retest);
            }
        }
        // Calendar: the app's own Microsoft 365 sign-in (read-only).
        let calendar = Submenu::new(if model.calendar_account.is_some() { "Calendar ✓" } else { "Calendar" }, true);
        match &model.calendar_account {
            Some(who) => {
                let _ = calendar.append(&MenuItem::new(format!("Microsoft 365: {who}"), false, None));
                let sync = self.item("Refresh now", true, Some(Action::CalendarSync));
                let out = self.item("Sign out", true, Some(Action::CalendarLogout));
                let _ = calendar.append_items(&[&sync, &out]);
            }
            None => {
                let login = self.item("Connect Microsoft 365…", true, Some(Action::CalendarLogin));
                let _ = calendar.append(&login);
            }
        }
        let (id, is_default) = &model.graph_client;
        let label = if *is_default { "App: CodeMie (default)".to_string() } else { format!("App: {}…", id.chars().take(8).collect::<String>()) };
        let _ = calendar.append_items(&[&sep(), &MenuItem::new(label, false, None)]);
        let change = self.item("Change app (client) ID…", true, Some(Action::CalendarClientId));
        let _ = calendar.append(&change);
        let _ = menu.append_items(&[&settings, &models_menu, &calendar, &claude, &ai, &health, &sep()]);
        let up = match &model.update {
            Some(v) => self.item(&format!("Update to {v}…"), true, Some(Action::Update)),
            None => self.item("Check for updates…", true, Some(Action::CheckUpdates)),
        };
        let _ = menu.append_items(&[&up, &sep()]);
        // Quit: stops the watcher (a recording in progress is finished and saved first).
        self.quit = self.item("Quit Echo", true, Some(Action::Quit));
        let _ = menu.append_items(&[&self.quit, &MenuItem::new(format!("Echo {}", env!("CARGO_PKG_VERSION")), false, None)]);

        self.tray.set_menu(Some(Box::new(menu)));
        self.model = Some(model);
        if let Some(s) = self.state.take() {
            self.apply(s); // re-render status/record/stop on the new items
        }
    }

    pub(super) fn refresh_model(&mut self, force: bool) {
        let force = force || self.refresh_owed;
        if !force && self.last_refresh.elapsed() < Duration::from_secs(5) {
            return;
        }
        // Rebuilding would close the menu being read: on the first tick after it closes.
        if menu_open() {
            self.refresh_owed = true;
            return;
        }
        self.refresh_owed = false;
        self.last_refresh = Instant::now();
        let m = load_model(&self.health);
        if self.model.as_ref() != Some(&m) {
            self.rebuild(m);
        }
    }

    pub(super) fn apply(&mut self, state: UiState) {
        let dark = taskbar_dark();
        if self.state.as_ref() == Some(&state) && self.dark == dark {
            return;
        }
        self.dark = dark;
        let (icon, template) = icon_for(&state);
        #[cfg(target_os = "macos")]
        let _ = if template { self.tray.set_icon_templated(Some(icon)) } else { self.tray.set_icon(Some(icon)) };
        #[cfg(not(target_os = "macos"))]
        let _ = (template, self.tray.set_icon(Some(icon)));
        let text = status_text(&state);
        self.status.set_text(&text);
        let _ = self.tray.set_tooltip(Some(format!("Echo — {text}")));
        match &state {
            UiState::Idle { in_call } => {
                self.record.set_text(if *in_call == Some(true) { "Record this call" } else { "Record next call" });
                self.record.set_enabled(true);
                self.stop.set_text("Stop recording");
                self.stop.set_enabled(false);
            }
            UiState::Armed => {
                self.record.set_enabled(false);
                self.stop.set_text("Cancel");
                self.stop.set_enabled(true);
            }
            UiState::Recording { memo, .. } => {
                self.record.set_enabled(false);
                self.stop.set_text(if *memo { "Stop the memo" } else { "Stop recording" });
                self.stop.set_enabled(true);
            }
        }
        // One recording at a time: a memo starts whenever nothing records.
        self.memo.set_enabled(!matches!(state, UiState::Recording { .. }));
        self.note.set_enabled(matches!(state, UiState::Recording { .. }));
        if self.quit.is_enabled() {
            self.quit.set_text(if matches!(state, UiState::Recording { .. }) {
                "Quit Echo (saves the current recording)"
            } else {
                "Quit Echo"
            });
        }
        self.state = Some(state);
    }

    /// "Update to …" (menu, or a click on the "Update available" notification): ask, download and
    /// verify it, then — once the window's notes are saved (`Leaving`) — swap it in and stop.
    pub(super) fn start_update(&self) {
        let Some(r) = crate::update::AVAILABLE.lock().unwrap().clone() else { return };
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let msg = format!("Install Echo {}?\n\nEcho restarts; nothing is recorded meanwhile.", r.version);
            if app::ask(&msg, &["Later", "Update"], "Update").as_deref() != Some("Update") {
                return;
            }
            match crate::update::prepare(&r) {
                Ok(staged) => drop(proxy.send_event(UiEvent::Leave(Then::Update(staged)))),
                Err(e) => app::notify("Update failed", &format!("{e:#}")),
            }
        });
    }

    /// The Quit item while quitting.
    pub(super) fn quitting(&mut self) {
        self.quit.set_text("Quitting…");
        self.quit.set_enabled(false);
    }

    pub(super) fn click(&mut self, id: &MenuId) {
        let Some(action) = self.actions.get(id).cloned() else { return };
        self.run(action)
    }

    /// The flows Settings in the window starts (`window::settings_action`), by name: the same
    /// actions as the menu's, so a dialog, a sign-in or a download runs one way only.
    pub(super) fn page_action(name: &str, arg: &str) -> Option<Action> {
        let model = || models::find(arg).ok().map(|m| m.name);
        Some(match name {
            "change_home" => Action::ChangeHome,
            "install_cli" => Action::InstallCli,
            "connect_claude" => Action::ConnectClaude,
            "claude_path" => Action::ClaudePath,
            "notes_choose" => Action::NotesChoose,
            "notes_anywhere" => Action::NotesAnywhere,
            "onedrive_choose" => Action::OnedriveChoose,
            "check_updates" => Action::CheckUpdates,
            "update" => Action::Update,
            "calendar_login" => Action::CalendarLogin,
            "calendar_logout" => Action::CalendarLogout,
            "calendar_sync" => Action::CalendarSync,
            "calendar_client_id" => Action::CalendarClientId,
            "use_model" => Action::UseModel(model()?),
            "remove_model" => Action::RemoveModel(model()?),
            "download_model" => Action::DownloadModel(model()?),
            "switch_model" => Action::SwitchModel(model()?),
            "edit_profile" => Action::EditProfile,
            "notes_model" => Action::NotesModel,
            "sign_in" => Action::SignIn(arg.to_string()),
            "ask_permissions" => Action::AskPermissions,
            "recheck" => Action::Recheck,
            "open_settings_url" => Action::OpenUrl(arg.to_string()),
            #[cfg(feature = "transcribe")]
            "connect_server" => Action::ConnectServer,
            #[cfg(feature = "transcribe")]
            "disconnect_server" => Action::DisconnectServer,
            #[cfg(feature = "transcribe")]
            "toggle_serving" => Action::ToggleServing,
            #[cfg(feature = "transcribe")]
            "retest_gpu" => Action::RetestGpu,
            _ => return None,
        })
    }

    pub(super) fn run(&mut self, action: Action) {
        match action {
            Action::Record => drop(watch::arm(None, None, None, None)),
            Action::Memo => {
                if let Err(e) = watch::memo_start(None, None) {
                    app::notify("Memo not started", &format!("{e:#}"));
                }
            }
            Action::RecordIn(code) => drop(watch::arm(None, None, Some(vec![code]), None)),
            Action::Stop => drop(watch::stop()),
            Action::ArmMeeting(m) => drop(watch::arm(m.subject.clone(), Some(*m), None, None)),
            Action::JoinMeeting(m) => drop(watch::join(*m)),
            Action::Open(p) => app::open_path(&p),
            Action::SetUnscheduled(v) => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.unscheduled = Some(v.to_string());
                let _ = cfg.save();
            }
            Action::SetScheduled(v) => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.scheduled = Some(v.to_string());
                let _ = cfg.save();
            }
            Action::ToggleFrames => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.capture_frames = !cfg.capture_frames;
                let _ = cfg.save();
                #[cfg(target_os = "macos")]
                if cfg.capture_frames && !crate::frames::permitted() {
                    // macOS shows its prompt once; afterwards it only lists the app in Settings.
                    std::thread::spawn(|| {
                        if !crate::frames::request() {
                            app::notify("Screen Recording needed", "Enable Echo in Privacy & Security → Screen & System Audio Recording, then restart Echo.");
                        }
                    });
                }
            }
            Action::ToggleKeepAudio => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.keep_audio = !cfg.keep_audio;
                let _ = cfg.save();
            }
            Action::ToggleMinutesPage => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.minutes_page = !cfg.minutes_page;
                let _ = cfg.save();
            }
            Action::ToggleAutoMinutes => {
                let saved = Config::load().and_then(|mut cfg| {
                    let on = !cfg.auto_minutes;
                    cfg.set_auto_minutes(on);
                    cfg.save()
                });
                if let Err(e) = saved {
                    app::notify("Couldn't change automatic minutes", &format!("{e:#}"));
                }
            }
            Action::UseModel(name) => drop(models::use_model(name)),
            Action::RemoveModel(name) => app::remove_model_asking(name),
            Action::NotesAnywhere => {
                let Ok(mut cfg) = Config::load() else { return };
                cfg.notes_in = None;
                let _ = cfg.save();
            }
            Action::OnedriveChoose => {
                std::thread::spawn(|| {
                    if let Some(dir) = app::choose_folder("The folder OneDrive keeps on this computer") {
                        let Ok(mut cfg) = Config::load() else { return };
                        match crate::onedrive::setting(&dir.to_string_lossy()) {
                            Ok(v) => {
                                cfg.onedrive = v;
                                let _ = cfg.save();
                            }
                            Err(e) => app::notify("OneDrive folder", &format!("{e:#}")),
                        }
                    }
                });
            }
            Action::NotesChoose => {
                std::thread::spawn(|| {
                    if let Some(dir) = app::choose_folder("Echo runs Claude Code for call minutes in this folder (its .claude/settings.json applies)") {
                        let Ok(mut cfg) = Config::load() else { return };
                        cfg.notes_in = Some(dunce::canonicalize(&dir).unwrap_or(dir));
                        let _ = cfg.save();
                    }
                });
            }
            Action::ConnectClaude => {
                std::thread::spawn(|| {
                    match app::connect_claude("project", None) {
                        Ok(()) => app::notify("Claude Code connected", "Claude Code lists Echo's plugin in Echo's data folder: its sessions get the status line and /echo:process-call."),
                        Err(e) => app::notify("Could not connect Claude Code", &format!("{e:#}")),
                    }
                    crate::claude_setup::status(true);
                });
            }
            Action::ClaudePath => {
                std::thread::spawn(|| {
                    match crate::claude_setup::add_to_path() {
                        Ok(what) => app::notify("Claude Code is on your PATH", &format!("{what} Open a new terminal to use `claude`.")),
                        Err(e) => app::notify("Couldn't add Claude Code to your PATH", &format!("{e:#}")),
                    }
                    crate::claude_setup::status(true);
                });
            }
            Action::Update => {
                self.start_update();
                return;
            }
            Action::CheckUpdates => {
                // Found: the same prompt as "Update to …" (the tray loop starts it).
                std::thread::spawn(|| match crate::update::check() {
                    Ok(Some(_)) => app::request_update(),
                    Ok(None) => app::notify("Echo is up to date", &format!("Echo {} is the latest version.", env!("CARGO_PKG_VERSION"))),
                    Err(e) => app::notify("Couldn't check for updates", &format!("{e:#}")),
                });
                return;
            }
            Action::CalendarLogin => {
                std::thread::spawn(|| match crate::graph::login() {
                    Ok(who) => {
                        let _ = crate::calendar::sync(true);
                        app::notify("Calendar connected", &who);
                    }
                    Err(e) => app::notify("Calendar sign-in failed", &format!("{e:#}")),
                });
                return;
            }
            Action::CalendarLogout => drop(crate::graph::logout()),
            Action::CalendarSync => {
                std::thread::spawn(|| drop(crate::calendar::sync(true)));
            }
            Action::CalendarClientId => {
                std::thread::spawn(|| {
                    let (current, _) = crate::graph::app_registration();
                    let msg = "Microsoft 365 app (client) ID for the calendar sign-in.\n\nYour IT can register a public client with redirect http://localhost and the delegated Calendars.Read permission. Type \"default\" for CodeMie.";
                    if let Some(id) = app::ask_text(msg, &current, "Save") {
                        let Ok(mut cfg) = Config::load() else { return };
                        let id = id.trim().to_string();
                        cfg.graph_client_id = (!id.eq_ignore_ascii_case("default") && id != crate::graph::DEFAULT_CLIENT_ID).then_some(id);
                        let _ = cfg.save();
                    }
                });
                return;
            }
            Action::ToggleLiveNotes => {
                let r = crate::ai::Settings::update(|s| {
                    s.live_notes = !s.live_notes;
                    Ok(())
                });
                if let Err(e) = r {
                    app::notify("Couldn't change the AI settings", &format!("{e:#}"));
                }
            }
            Action::DefaultProfile(id) => {
                // `update` refuses a default that was removed meanwhile (the menu was open).
                let r = crate::ai::Settings::update(|s| {
                    s.default = (id != crate::ai::SYSTEM).then_some(id);
                    Ok(())
                });
                if let Err(e) = r {
                    app::notify("Couldn't change the AI settings", &format!("{e:#}"));
                }
            }
            Action::AcceptAccount(id, shown) => {
                crate::signin::accept(&id, &shown); // checks off the menu's thread
                return;
            }
            Action::SignIn(id) => {
                crate::signin::start(&id); // the browser opens; Echo notices when it's done
                return;
            }
            Action::NotesModel => {
                // The model that writes the minutes of the profile new calls use (`ai::set_notes_model`).
                std::thread::spawn(|| {
                    let p = crate::ai::Settings::load().and_then(|s| s.get(&s.default_id()));
                    let p = match p {
                        Ok(p) => p,
                        Err(e) => return app::notify("Echo", &format!("The minutes model can't be changed: {e:#}")),
                    };
                    let (current, how) = match p.kind {
                        crate::ai::Kind::System => (
                            Config::load().unwrap_or_default().notes_model_label(),
                            "sonnet, opus or haiku, a full model name, or \"default\" to use Claude Code's own setting (local models, a gateway, Bedrock or Vertex).",
                        ),
                        crate::ai::Kind::Claude => (p.notes_model_label(), "sonnet, opus or haiku, or a full model name; empty for sonnet."),
                        crate::ai::Kind::Local => (p.notes_model_label(), "the model's name on the server; empty for its live model."),
                    };
                    let msg = format!("Model that writes the minutes of {} (the profile new calls use):\n\n{how}", p.name);
                    if let Some(m) = app::ask_text(&msg, &current, "Save") {
                        if let Err(e) = crate::ai::set_notes_model(&p.id, &m) {
                            app::notify("Echo", &format!("The minutes model wasn't changed: {e:#}"));
                        }
                    }
                });
                return;
            }
            Action::OpenWindow(call) => {
                self.open_window = Some(call);
                return;
            }
            Action::AddNote => {
                // Your own note: it becomes the backbone of the call's minutes ("# …" = heading).
                std::thread::spawn(|| {
                    if let Some(t) = app::ask_text("Note for this call (start with # for a heading):", "", "Add") {
                        if let Err(e) = crate::calls::note(&t) {
                            app::notify("Note not saved", &format!("{e:#}"));
                        }
                    }
                });
                return;
            }
            Action::EditProfile => {
                if let Ok(p) = crate::calls::ensure_me_profile() {
                    app::open_path(&p);
                }
            }
            Action::InstallCli => {
                std::thread::spawn(|| match app::install_cli() {
                    Ok(()) => app::notify("Command-line tool installed", "Type `echoctl` in a new terminal."),
                    Err(e) => app::notify("Command-line tool not installed", &format!("{e:#}")),
                });
            }
            Action::Recheck => {
                self.health = health();
                crate::claude_setup::status(true);
            }
            Action::AskPermissions => app::request_permissions(),
            Action::ConnectServer => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(connect_server);
                return;
            }
            Action::DisconnectServer => {
                #[cfg(feature = "transcribe")]
                if let Err(e) = crate::remote::disconnect() {
                    app::notify("Transcription server", &format!("{e:#}"));
                }
            }
            Action::ToggleServing => {
                #[cfg(feature = "transcribe")]
                {
                    let on = !crate::remote::serve_settings().on;
                    match crate::remote::set_serving(on, false) {
                        Ok(s) if s.on => drop(std::thread::spawn(move || show_serving(&s))),
                        Ok(_) => {}
                        Err(e) => app::notify("Transcription server", &format!("{e:#}")),
                    }
                }
            }
            Action::ShowServing => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(|| show_serving(&crate::remote::serve_settings()));
                return;
            }
            Action::RetestGpu => {
                #[cfg(feature = "transcribe")]
                std::thread::spawn(|| {
                    let ok = crate::gpu::retest();
                    app::notify("GPU test", if ok { "Transcription runs on the GPU." } else { "The GPU test failed: transcription runs on the CPU." });
                });
                return;
            }
            Action::DownloadModel(name) => models::download_in_background(name),
            Action::SwitchModel(name) => models::switch_in_background(name),
            Action::ToggleAutostart => drop(app::set_autostart(!app::autostart_enabled())),
            Action::ChangeHome => {
                // The picker blocks: run it off the event loop, then restart on the new folder once
                // the window's notes are saved (`Leaving`).
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let Some(dir) = app::choose_folder("Folder for Echo calls and minutes") else { return };
                    if dir != paths::home() {
                        let _ = proxy.send_event(UiEvent::Leave(Then::ChangeHome(dir)));
                    }
                });
                return;
            }
            Action::OpenUrl(u) => drop(permissions::open_settings(&u)),
            Action::Quit => {
                let _ = self.proxy.send_event(UiEvent::Leave(Then::Quit));
                return;
            }
        }
        self.refresh_model(true);
    }
}
