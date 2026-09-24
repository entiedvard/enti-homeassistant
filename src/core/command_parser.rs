use crate::core::models::{
    Action,
    CommandIntent,
};


pub struct CommandParser;


impl CommandParser {


    pub fn parse(
        command: &str
    ) -> CommandIntent {


        let text =
            command.to_lowercase();



        let action =
            Self::detect_action(
                &text
            );



        let target =
            Self::extract_target(
                &text
            );



        CommandIntent {

            action,

            target,

        }

    }



    fn detect_action(
        text: &str
    ) -> Action {


        if text.contains("включ")
            || text.contains("зажги")
            || text.contains("запусти")
        {

            return Action::TurnOn;

        }



        if text.contains("выключ")
            || text.contains("отключ")
        {

            return Action::TurnOff;

        }



        if text.contains("переключ")
        {

            return Action::Toggle;

        }



        Action::Unknown

    }



    fn extract_target(
        text: &str
    ) -> String {


        let words: Vec<&str> =
            text.split_whitespace()
            .collect();



        let stop_words = [
            "включи",
            "включить",
            "выключи",
            "выключить",
            "отключи",
            "переключи",
            "запусти",
            "зажги",
            "пожалуйста",
        ];



        let mut target =
            Vec::new();



        for word in words {


            if !stop_words.contains(&word) {

                target.push(word);

            }

        }



        target.join(" ")

    }

}