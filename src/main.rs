use std::{error::Error, io};

mod build_log;
use build_log::BuildLogEntry;

use std::sync::mpsc;
use std::thread;

use std::io::BufRead;

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

    let log_receiver = spawn_reader();

    // create app and run it
    let res = App::new(log_receiver).run(&mut terminal);

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

//TODO spawn ninja as subprocess, then we can read directly from it's stdout
fn spawn_reader() -> mpsc::Receiver<BuildLogEntry> {
    let (tx, rx) = mpsc::channel::<BuildLogEntry>();
    thread::spawn(move || {
        let filename = std::env::args().skip(1).next().unwrap();
        let file = std::fs::File::open(filename).unwrap();
        for line in std::io::BufReader::new(file).lines() {
            let entry = serde_json::from_str(&line.unwrap()).expect("Could not parse json");
            tx.send(entry).unwrap();
        }
    });
    rx
}

fn entry_color(success: &bool) -> Color {
    match success {
        true => Color::Reset,
        false => Color::Red,
    }
}

fn log_entry_to_list_item(item: &BuildLogEntry) -> ListItem {
    match item {
        BuildLogEntry::BuildEdgeFinished {
            edge_id: _,
            success,
            command,
            output: _,
        } => {
            let style = Style::default().bg(entry_color(success));
            let text = Text::styled(command, style);
            ListItem::new(text)
        }
    }
}

fn log_entry_to_output(item: &BuildLogEntry) -> String {
    match item {
        BuildLogEntry::BuildEdgeFinished {
            edge_id: _,
            success: _,
            command: _,
            output,
        } => output.clone(),
    }
}

struct App {
    log_entries: Vec<BuildLogEntry>,
    state: ListState,
    log_receiver: mpsc::Receiver<BuildLogEntry>,
}

impl App {
    fn new(log_receiver: mpsc::Receiver<BuildLogEntry>) -> App {
        App {
            log_entries: Vec::new(),
            state: ListState::default().with_selected(Some(0)),
            log_receiver,
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
            for entry in self.log_receiver.try_iter() {
                self.log_entries.push(entry);
            }

            self.draw(terminal)?;

            // TODO event::read blocks until the next event, so no new entries are received until a key is pressed
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

        let list = List::new(self.log_entries.iter().map(log_entry_to_list_item))
            .block(Block::default().title("Log entries").borders(Borders::ALL))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ")
            .repeat_highlight_symbol(true);

        frame.render_stateful_widget(list, log_list_area, &mut self.state);

        let selected_output: String = self
            .state
            .selected()
            .and_then(|i| self.log_entries.get(i))
            .map(log_entry_to_output)
            .unwrap_or(String::new());
        let output_par =
            Paragraph::new(selected_output).block(Block::bordered().title("Log Output"));
        frame.render_widget(output_par, log_output_area);

        frame.render_widget(Block::bordered().title("Dependencies"), dependency_area);
    }
}
