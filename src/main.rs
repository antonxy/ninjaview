use itertools::Itertools;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{error::Error, io};

use std::process;

mod build_log;
use build_log::{BuildLogEntry, BuildState, StructLogMessage};

use std::sync::mpsc;
use std::thread;

use std::io::{BufRead, Read};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};

use clap::{Args, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    log_file: Option<PathBuf>,

    #[command(flatten)]
    ninja_args: NinjaArgs,
}

#[derive(Args, Debug)]
struct NinjaArgs {
    #[arg(long)]
    ninja_binary: Option<PathBuf>,

    #[arg(short, long)]
    build_dir: Option<PathBuf>,

    #[arg(trailing_var_arg = true)]
    ninja_args: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let log_receiver = match args.log_file {
        Some(log_file_path) => spawn_file_reader(&log_file_path),
        None => spawn_ninja(args.ninja_args),
    };

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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

fn spawn_file_reader(filename: &Path) -> mpsc::Receiver<StructLogMessage> {
    let file = std::fs::File::open(filename).unwrap();
    spawn_reader(file)
}

fn spawn_ninja(args: NinjaArgs) -> mpsc::Receiver<StructLogMessage> {
    let mut ninja = process::Command::new(args.ninja_binary.unwrap_or("ninja".into()))
        .current_dir(args.build_dir.unwrap_or(PathBuf::from(".")))
        .arg("-d")
        .arg("structlog")
        .args(args.ninja_args)
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn ninja process");

    let output = ninja.stdout.take().unwrap();
    spawn_reader(output)
}

fn spawn_reader<R: Read + Send + 'static>(reader: R) -> mpsc::Receiver<StructLogMessage> {
    let (tx, rx) = mpsc::channel::<StructLogMessage>();
    thread::spawn(move || {
        for line in std::io::BufReader::new(reader).lines() {
            let entry = serde_json::from_str(&line.unwrap()).expect("Could not parse json");
            tx.send(entry).unwrap();
        }
    });
    rx
}

fn entry_color(success: Option<bool>) -> Color {
    match success {
        Some(true) | None => Color::Reset,
        Some(false) => Color::Red,
    }
}

fn log_entry_to_list_item(item: &BuildLogEntry) -> ListItem {
    let style = Style::default().bg(entry_color(item.success));
    let inputs: String = item
        .inputs
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap())
        .intersperse(", ")
        .collect();
    let outputs: String = item
        .outputs
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap())
        .intersperse(", ")
        .collect();
    let string: String = format!("{}: {} -> {}", item.compiler, inputs, outputs);
    let text = Text::styled(string.to_owned(), style);
    ListItem::new(text)
}

enum UIEvent {
    BuildLog(StructLogMessage),
    UserAction(crossterm::event::Event),
}

struct App {
    build_state: BuildState,
    list_state: ListState,
    log_receiver: mpsc::Receiver<StructLogMessage>,
}

impl App {
    fn new(log_receiver: mpsc::Receiver<StructLogMessage>) -> App {
        App {
            build_state: BuildState::new(),
            list_state: ListState::default().with_selected(Some(0)),
            log_receiver,
        }
    }

    fn select_log(&mut self, offset: isize) {
        if self.build_state.log_entries.is_empty() {
            self.list_state.select(None);
        } else {
            let selected = self.list_state.selected().unwrap_or(0);
            let new = usize::saturating_add_signed(selected, offset)
                .min(self.build_state.log_entries.len() - 1);
            self.list_state.select(Some(new));
        }
    }

    fn read_event(&mut self) -> io::Result<UIEvent> {
        loop {
            match self.log_receiver.try_recv() {
                Ok(event) => return Ok(UIEvent::BuildLog(event)),
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {}
            };
            if event::poll(Duration::from_millis(100))? {
                return Ok(UIEvent::UserAction(event::read()?));
            };
        }
    }

    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        loop {
            match self.read_event() {
                Ok(UIEvent::BuildLog(msg)) => {
                    self.build_state.update(msg);
                }
                Ok(UIEvent::UserAction(Event::Key(key))) => {
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
                Ok(UIEvent::UserAction(_)) => {}
                Err(e) => return Err(e),
            }

            self.draw(terminal)?;
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| self.ui(f))?;
        Ok(())
    }

    fn ui(&mut self, frame: &mut Frame) {
        let [main_area, status_area] = Layout::vertical([Min(0), Length(1)]).areas(frame.size());

        frame.render_widget(
            Paragraph::new(vec![Line::from(
                format!(
                    " {} - {} / {}",
                    self.build_state.build_status.to_string(),
                    self.build_state.log_entries.len(),
                    self.build_state.total_edges
                )
                .dark_gray(),
            )]),
            status_area,
        );

        let [log_area, dependency_area] =
            Layout::horizontal([Percentage(70), Percentage(30)]).areas(main_area);

        let [log_list_area, log_output_area] =
            Layout::vertical([Percentage(50), Percentage(50)]).areas(log_area);

        let list = List::new(
            self.build_state
                .log_entries
                .iter()
                .map(log_entry_to_list_item),
        )
        .block(Block::default().title("Log entries").borders(Borders::ALL))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ")
        .repeat_highlight_symbol(true);

        frame.render_stateful_widget(list, log_list_area, &mut self.list_state);

        let selected_output: String = self
            .list_state
            .selected()
            .and_then(|i| self.build_state.log_entries.get(i))
            .and_then(|e| e.output.clone())
            .unwrap_or(String::new());
        let output_par =
            Paragraph::new(selected_output).block(Block::bordered().title("Log Output"));
        frame.render_widget(output_par, log_output_area);

        frame.render_widget(Block::bordered().title("Dependencies"), dependency_area);
    }
}
