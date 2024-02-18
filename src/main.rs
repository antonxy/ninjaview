use std::{error::Error, io};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = App::new().run(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

struct LogEntry {
    commandline: String,
    status_code: u8,
    output: String,
}

impl LogEntry {
    fn entry_color(&self) -> Color {
        match self.status_code {
            0 => Color::Reset,
            _ => Color::Red,
        }
    }
    fn to_list_item(&self) -> ListItem {
        let style = Style::default().bg(self.entry_color());
        let text = Text::styled(self.commandline.clone(), style);
        ListItem::new(text)
    }
}

struct App {
    log_entries: Vec<LogEntry>,
    state: ListState,
}

impl App {
    fn new() -> App {
        App {
            log_entries: demo_log(),
            state: ListState::default().with_selected(Some(0)),
        }
    }

    fn select_log(&mut self, offset: isize) {
        if self.log_entries.is_empty() {
            self.state.select(None);
        } else {
            let selected = self.state.selected().unwrap_or(0);
            let new =
                usize::saturating_add_signed(selected, offset).min(self.log_entries.len() - 1);
            self.state.select(Some(new));
        }
    }

    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(terminal)?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    match key.code {
                        Char('q') | Esc => return Ok(()),
                        Char('j') | Down => self.select_log(1),
                        Char('k') | Up => self.select_log(-1),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| self.ui(f))?;
        Ok(())
    }

    fn ui(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Length(1), Min(0)]);
        let [title_area, main_area] = vertical.areas(frame.size());

        frame.render_widget(
            Paragraph::new(vec![
                Line::from("Ninja structured log viewer".dark_gray()).centered()
            ]),
            title_area,
        );

        let [log_area, dependency_area] =
            Layout::horizontal([Percentage(70), Percentage(30)]).areas(main_area);

        let [log_list_area, log_output_area] =
            Layout::vertical([Percentage(50), Percentage(50)]).areas(log_area);

        let list = List::new(self.log_entries.iter().map(LogEntry::to_list_item))
            .block(Block::default().title("Log entries").borders(Borders::ALL))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ")
            .repeat_highlight_symbol(true);

        frame.render_stateful_widget(list, log_list_area, &mut self.state);

        let selected_output = self
            .state
            .selected()
            .and_then(|i| self.log_entries.get(i))
            .map(|e| e.output.clone())
            .unwrap_or(String::new());
        let output_par =
            Paragraph::new(selected_output).block(Block::bordered().title("Log Output"));
        frame.render_widget(output_par, log_output_area);

        frame.render_widget(Block::bordered().title("Dependencies"), dependency_area);
    }
}

fn demo_log() -> Vec<LogEntry> {
    vec![
        LogEntry {
            commandline: "g++ main.cpp".to_owned(),
            status_code: 0,
            output: "".to_owned(),
        },
        LogEntry {
            commandline: "g++ test.cpp".to_owned(),
            status_code: 0,
            output: "".to_owned(),
        },
        LogEntry {
            commandline: "g++ something.cpp".to_owned(),
            status_code: 1,
            output: "ERROR at line 123".to_owned(),
        },
    ]
}
