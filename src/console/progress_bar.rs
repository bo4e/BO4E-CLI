use crate::console::palette;
use indicatif::ProgressStyle;
use lazy_static::lazy_static;
use octocrab::models::code_scannings::Message;

lazy_static! {
    static ref PROGRESS_BAR_STYLE_RUNNING: ProgressStyle = ProgressStyle::with_template(&format!(
        "{{msg}} {{bar:40.{}/{}}} {{pos:>7}}/{{len:7}} {{elapsed_precise}}",
        palette::ERROR,
        palette::SUB
    ))
    .unwrap();
    static ref PROGRESS_BAR_STYLE_FINISHED: ProgressStyle = ProgressStyle::with_template(&format!(
        "{{msg}} {{bar:40.{}/{}}} {{pos:>7}}/{{len:7}} {{elapsed_precise}}",
        palette::MAIN,
        palette::SUB
    ))
    .unwrap();
}

pub fn new_progress_bar(total: u64, message: Option<String>) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(total);
    pb.set_style(PROGRESS_BAR_STYLE_RUNNING.clone());
    if let Some(msg) = message {
        pb.set_message(msg);
    }
    format!("");
    pb
}

pub fn finish_progress_bar(pb: &indicatif::ProgressBar, message: Option<String>) {
    pb.set_style(PROGRESS_BAR_STYLE_FINISHED.clone());
    if let Some(msg) = message {
        pb.finish_with_message(msg);
    } else {
        pb.finish();
    }
}

pub fn abandon_progress_bar_with_error(pb: &indicatif::ProgressBar, message: String) {
    pb.abandon_with_message(message);
}
