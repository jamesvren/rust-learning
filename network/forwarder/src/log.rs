//use systemd_journal_logger::JournalLog;
use anyhow::Result;
use log::LevelFilter;
use log4rs::{
    append::rolling_file::{
        policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        RollingFileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::fs;
use std::path::Path;

pub fn init(dir: &str, size: u64, max: u32) -> Result<()> {
    //env_logger::init();

    //JournalLog::new()?
    //    .install()?;
    //log::set_max_level(LevelFilter::Info);

    let pattern = ensure_log_dir(dir)?;
    let trigger = SizeTrigger::new(size * 1024_u64 * 1024_u64);
    let roller = FixedWindowRoller::builder().build(&pattern, max).unwrap();
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let logfile = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S%.3f %Z)(utc)} {l} - {m}{n}",
        )))
        .build(format!("{dir}/forwarder.log"), Box::new(policy))
        .unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))
        .unwrap();
    let _handle = log4rs::init_config(config)?;
    Ok(())
}

fn ensure_log_dir(dir: &str) -> Result<String> {
    let path = Path::new(dir);
    if !path.try_exists()? {
        fs::create_dir_all(dir)?;
    }
    Ok(format!("{}forwarder.{{}}.gz", path.to_str().unwrap()))
}
