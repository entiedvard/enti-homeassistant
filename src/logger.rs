use std::str::FromStr;
use std::sync::OnceLock;



#[derive(
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Debug
)]
pub enum LogLevel {

    Silent = 0,

    Error = 1,

    Info = 2,

    Debug = 3,

}



impl FromStr for LogLevel {

    type Err = ();


    fn from_str(
        value: &str
    ) -> Result<Self, Self::Err> {


        match value
            .to_lowercase()
            .as_str()
        {


            "debug" =>
                Ok(LogLevel::Debug),



            "info" =>
                Ok(LogLevel::Info),



            "error" =>
                Ok(LogLevel::Error),



            "silent" =>
                Ok(LogLevel::Silent),



            _ =>
                Err(()),

        }

    }

}



static CURRENT_LEVEL: OnceLock<LogLevel> =
    OnceLock::new();



pub fn set_level(
    level: LogLevel
) {

    CURRENT_LEVEL
        .set(level)
        .ok();

}



fn current_level() -> LogLevel {


    match CURRENT_LEVEL.get() {


        Some(level) =>
            *level,


        None =>
            LogLevel::Info,

    }

}





pub fn log(
    level: LogLevel,
    message: &str
) {


    if level == LogLevel::Silent {
        return;
    }



    if level <= current_level() {


        let prefix =
            match level {


                LogLevel::Error =>
                    "[ERROR]",



                LogLevel::Info =>
                    "[INFO]",



                LogLevel::Debug =>
                    "[DEBUG]",



                LogLevel::Silent =>
                    "",

            };



        println!(
            "{} {}",
            prefix,
            message
        );

    }

}





pub fn error(
    msg: &str
) {

    log(
        LogLevel::Error,
        msg
    );

}





pub fn info(
    msg: &str
) {

    log(
        LogLevel::Info,
        msg
    );

}





pub fn debug(
    msg: &str
) {

    log(
        LogLevel::Debug,
        msg
    );

}