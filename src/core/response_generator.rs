use crate::core::models::CommandResult;


pub struct ResponseGenerator;


impl ResponseGenerator {


    pub fn generate(
        result: &CommandResult
    ) -> String {


        match result {


            CommandResult::Success {
                action,
                device_name,
                already_state,
                confirmed_state,
                ..
            } => {


                if *already_state {

                    return match action {


                        crate::core::models::Action::TurnOn => {

                            format!(
                                "{} уже включён",
                                device_name
                            )

                        }


                        crate::core::models::Action::TurnOff => {

                            format!(
                                "{} уже выключен",
                                device_name
                            )

                        }


                        crate::core::models::Action::Toggle => {

                            format!(
                                "{} уже был переключён",
                                device_name
                            )

                        }


                        _ => {

                            format!(
                                "{} уже имеет нужное состояние",
                                device_name
                            )

                        }

                    };

                }



                match action {


                    crate::core::models::Action::TurnOn => {

                        format!(
                            "Включаю {}",
                            device_name
                        )

                    }


                    crate::core::models::Action::TurnOff => {

                        format!(
                            "Выключаю {}",
                            device_name
                        )

                    }


                    crate::core::models::Action::Toggle => {

                        format!(
                            "Переключаю {}",
                            device_name
                        )

                    }


                    _ => {

                        format!(
                            "Выполняю действие с {}",
                            device_name
                        )

                    }

                }


            }



            CommandResult::DeviceNotFound {
                target
            } => {

                format!(
                    "Устройство {} не найдено",
                    target
                )

            }



            CommandResult::MultipleDevices {
                count
            } => {

                format!(
                    "Найдено несколько устройств: {}. Уточните команду",
                    count
                )

            }



            CommandResult::ServiceError {
                error
            } => {

                format!(
                    "Не удалось выполнить команду: {}",
                    error
                )

            }



            CommandResult::Timeout {
                device_name,
                ..
            } => {

                format!(
                    "Не удалось дождаться ответа от {}",
                    device_name
                )

            }

            CommandResult::ConnectionError {
                    error
                } => {

                    format!(
                        "Не могу выполнить команду: {}",
                        error
                    )

            }


        }

    }


}