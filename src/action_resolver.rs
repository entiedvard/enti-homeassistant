use crate::core::models::Action;


pub struct ActionResolver;



impl ActionResolver {


    pub fn resolve(
        command: &str
    ) -> Action {


        let text =
            command.to_lowercase();



        if text.contains("включ")
            || text.contains("зажг")
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
            || text.contains("выкл")
        {

            return Action::Toggle;

        }



        Action::Unknown

    }


}