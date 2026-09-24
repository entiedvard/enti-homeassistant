use std::collections::HashMap;

use crate::models::{
    DeviceState,
    SearchResult,
    DomainFilter,
    SearchConfidence,
    ResolveResult,
};


pub struct EntityCache {

    pub entities: HashMap<String, DeviceState>,

}



impl EntityCache {


    pub fn new() -> Self {

        Self {

            entities: HashMap::new(),

        }

    }



    pub fn count(&self) -> usize {

        self.entities.len()

    }



    pub fn insert(
        &mut self,
        device: DeviceState
    ) {

        self.entities.insert(
            device.entity_id.clone(),
            device
        );

    }



    pub fn update(
        &mut self,
        entity_id: String,
        state: String
    ) {


        if let Some(device) =
            self.entities.get_mut(&entity_id)
        {

            device.state = state;

        }

    }



    pub fn get(
        &self,
        entity_id: &str
    ) -> Option<&DeviceState> {


        self.entities.get(entity_id)

    }





    fn domain_match(
        device: &DeviceState,
        filter: &DomainFilter
    ) -> bool {


        match filter {


            DomainFilter::Any =>
                true,


            DomainFilter::Light =>
                device.entity_id.starts_with("light."),


            DomainFilter::Switch =>
                device.entity_id.starts_with("switch."),


            DomainFilter::Sensor =>
                device.entity_id.starts_with("sensor."),


            DomainFilter::Automation =>
                device.entity_id.starts_with("automation."),


        }

    }





    fn calculate_score(
        device: &DeviceState,
        query: &str
    ) -> u32 {


        let query =
            query.to_lowercase();


        let name =
            device.friendly_name
            .to_lowercase();


        let id =
            device.entity_id
            .to_lowercase();



        let mut score = 0;



        // Полное совпадение имени

        if name == query {

            score += 60;

        }

        // Имя содержит запрос

        else if name.contains(&query) {

            score += 45;

        }



        // Совпадение entity_id

        if id.contains(&query) {

            score += 30;

        }



        // Совпадение отдельных слов

        for word in query.split_whitespace() {


            if name.contains(word) {

                score += 10;

            }


        }



        score.min(100)

    }







    pub fn confidence(
        score: u32
    ) -> SearchConfidence {


        match score {


            80..=100 =>
                SearchConfidence::High,


            50..=79 =>
                SearchConfidence::Medium,


            _ =>
                SearchConfidence::Low,


        }

    }







    pub fn search(
        &self,
        query: &str,
        filter: DomainFilter
    ) -> Vec<SearchResult> {


        let mut results =
            Vec::new();



        for device in self.entities.values() {


            if !Self::domain_match(
                device,
                &filter
            ) {

                continue;

            }



            let score =
                Self::calculate_score(
                    device,
                    query
                );



            if score > 0 {


                results.push(
                    SearchResult {

                        device,

                        score,

                    }
                );

            }


        }




        results.sort_by(
            |a,b|
            b.score.cmp(&a.score)
        );



        results

    }







    pub fn resolve(
        &self,
        query: &str,
        filter: DomainFilter
    ) -> ResolveResult {


        let results =
            self.search(
                query,
                filter
            );



        if results.is_empty() {


            return ResolveResult::NotFound;


        }



        let best =
            &results[0];



        if best.score >= 85 {


            return ResolveResult::Found {device:
                    best.device,

                score:
                    best.score,

            };

        }



        if best.score >= 50 {


            return ResolveResult::Multiple {

                devices:
                    results,

            };

        }



        ResolveResult::NotFound


    }







    pub fn find(
        &self,
        query: &str
    ) -> Vec<&DeviceState> {


        let query =
            query.to_lowercase();



        let mut result =
            Vec::new();



        for device in self.entities.values() {


            if device.friendly_name
                .to_lowercase()
                .contains(&query)

                ||

                device.entity_id
                .to_lowercase()
                .contains(&query)

            {

                result.push(device);

            }


        }



        result

    }







    pub fn print_all(
        &self
    ) {


        for device in self.entities.values() {


            println!(
                "{} = {}",
                device.entity_id,
                device.state
            );


        }


    }


}