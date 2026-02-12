use std::thread;

use flexi_logger::DeferredNow;
// use flexi_logger::TS_DASHES_BLANK_COLONS_DOT_BLANK;
use flexi_logger::style;
use log::Record;

pub fn cli_format(
    w: &mut dyn std::io::Write,
    _now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    let level = record.level();
    write!(
        w,
        "{} [{}] {}",
        style(level).paint(format!("{:5}", record.level().to_string())),
        // now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        style(level).paint(thread::current().name().unwrap_or("<unnamed>")),
        // record.level(),
        &record.args()
    )
}
