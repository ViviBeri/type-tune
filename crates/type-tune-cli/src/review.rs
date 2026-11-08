//! Interactive review command.

use std::{fs, process::ExitCode};

use crate::{
    help::print_review_help,
    options::{REVIEW_OPTIONS, has_help_flag, option_value, validate_options},
    output::read_text_file,
};

/// Runs the interactive Markdown review workflow.
pub(crate) fn run_review() -> ExitCode {
    let arguments = std::env::args().skip(2).collect::<Vec<_>>();
    if has_help_flag(&arguments) {
        print_review_help();
        return ExitCode::SUCCESS;
    }
    if let Err(error) = validate_options(
        &arguments,
        REVIEW_OPTIONS.value_options,
        REVIEW_OPTIONS.flag_options,
    ) {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let Some(markdown_path) = option_value(&arguments, "--markdown") else {
        eprintln!("review requires --markdown <path>");
        return ExitCode::from(2);
    };
    if !crossterm::tty::IsTty::is_tty(&std::io::stdin())
        || !crossterm::tty::IsTty::is_tty(&std::io::stdout())
    {
        eprintln!("review requires an interactive TTY.");
        return ExitCode::FAILURE;
    }
    let content = match read_text_file(std::path::Path::new(markdown_path)) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let document = match type_tune_core::review::parse_review_plan_markdown(&content) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if document.items.is_empty() {
        eprintln!("No reviewable candidates found in {markdown_path}.");
        return ExitCode::FAILURE;
    }
    let mut state = type_tune_core::review::ReviewState {
        document,
        selected_index: 0,
        previous_key_was_enter: false,
        saved: false,
    };
    if let Err(error) = run_interactive_review(&mut state, markdown_path) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run_interactive_review(
    state: &mut type_tune_core::review::ReviewState,
    markdown_path: &str,
) -> Result<(), String> {
    use crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute, queue,
        style::Print,
        terminal::{self, ClearType},
    };
    use std::io::{Write, stdout};

    terminal::enable_raw_mode().map_err(|error| error.to_string())?;
    let mut stdout = stdout();
    let result = (|| -> Result<(), String> {
        loop {
            queue!(
                stdout,
                terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0),
                cursor::Hide,
                Print(type_tune_core::review::render_review_screen(state))
            )
            .map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;

            let Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) = event::read().map_err(|error| error.to_string())?
            else {
                continue;
            };
            if kind != KeyEventKind::Press {
                continue;
            }
            let key = match (code, modifiers) {
                (KeyCode::Esc, _) => "\u{001b}".to_owned(),
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => "\u{0003}".to_owned(),
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => "\u{0004}".to_owned(),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => "\u{0015}".to_owned(),
                (KeyCode::Char(' '), _) => " ".to_owned(),
                (KeyCode::Char('j'), _) => "j".to_owned(),
                (KeyCode::Char('k'), _) => "k".to_owned(),
                (KeyCode::Down, _) => "\u{001b}[B".to_owned(),
                (KeyCode::Up, _) => "\u{001b}[A".to_owned(),
                (KeyCode::Tab, KeyModifiers::SHIFT) => "\u{001b}[Z".to_owned(),
                (KeyCode::Tab, _) => "\t".to_owned(),
                (KeyCode::PageDown, _) => "\u{001b}[6~".to_owned(),
                (KeyCode::PageUp, _) => "\u{001b}[5~".to_owned(),
                (KeyCode::Enter, _) => "\r".to_owned(),
                _ => continue,
            };
            let action = type_tune_core::review::handle_review_key(state, &key);
            if key == "\r" || key == "\n" {
                let rendered =
                    type_tune_core::review::render_reviewed_plan_markdown(&state.document)
                        .map_err(|error| error.to_string())?;
                fs::write(markdown_path, rendered).map_err(|error| error.to_string())?;
            }
            if action == type_tune_core::review::ReviewKeyAction::Exit {
                break;
            }
        }
        Ok(())
    })();
    let _ = execute!(stdout, cursor::Show);
    let _ = terminal::disable_raw_mode();
    result
}
