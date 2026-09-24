use std::{collections::HashMap, io::SeekFrom};
use crate::logger::debug;

use crate::core::models::{
 DeviceState, DomainFilter, ResolveResult, ScoreReason, SearchConfidence, SearchResult,
};


pub struct EntityCache {

    pub entities: HashMap<String, DeviceState>,

}



impl EntityCache {

    pub fn len(
        &self
    ) ->usize {
        self.entities.len()
    }


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


pub fn get_all(
    &self
) -> Vec<DeviceState>
{

    self.entities
        .values()
        .cloned()
        .collect()

}


fn domain_match(
    device: &DeviceState,
    filter: &DomainFilter
) -> bool {


    let domain =
        device.entity_id
        .split('.')
        .next()
        .unwrap_or("");



    match filter {


        DomainFilter::Any => true,


        DomainFilter::Light => {
            domain == "light"
        }


        DomainFilter::Switch => {
            domain == "switch"
        }


        DomainFilter::Sensor => {
            domain == "sensor"
        }


        DomainFilter::Automation => {
            domain == "automation"
        }


    }

}

    fn get_domain(
    entity_id: &str
    ) -> String {

    entity_id
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()

    }


  fn calculate_score(
        device: &DeviceState,
        query: &str,
        filter: &DomainFilter
        ) -> (u32, Vec<ScoreReason>) {


    let query =
        query.to_lowercase();


    let name =
        device.friendly_name.to_lowercase();


    let id =
        device.entity_id.to_lowercase();



    let mut score = 0;

    let mut reasons = Vec::new();

   
    if name == query {

        score += 60;

        reasons.push(
            ScoreReason {
                points: 60,
                reason:
                "полное совпадение имени".into(),
            }
        );

    }

    else if name.contains(&query) {

        score += 45;

        reasons.push(
            ScoreReason {
                points:45,
                reason:
                "имя содержит запрос".into(),
            }
        );

    }



    if id.contains(&query) {

        score += 30;

        reasons.push(
            ScoreReason {
                points:30,
                reason:
                "совпадение entity_id".into(),
            }
        );

    }



    if Self::domain_match(
        device,
        filter
    ) {

        score += 15;

        reasons.push(
            ScoreReason {
                points:15,
                reason:
                "подходящий домен".into(),
            }
        );

    }



    (score.min(100), reasons)

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

    let query =
        query.to_lowercase();


    let mut results =
        Vec::new();



    for device in self.entities.values() {


        if !filter.matches(
            &device.domain
        ) {

            continue;

        }



        let mut score =
            0;


        let mut reasons =
            Vec::new();



        let name =
            device
            .friendly_name
            .to_lowercase();


        let id =
            device
            .entity_id
            .to_lowercase();



        if name == query {

            score += 60;

            reasons.push(
                ScoreReason {
                    points: 60,
                    reason:
                    "точное имя".to_string(),
                }
            );

        }


        else if name.contains(&query) {

            score += 40;

            reasons.push(
                ScoreReason {
                    points: 40,
                    reason:
                    "совпадение имени".to_string(),
                }
            );

        }



        if id.contains(&query) {

            score += 30;

            reasons.push(
                ScoreReason {
                    points: 30,
                    reason:
                    "совпадение entity_id".to_string(),
                }
            );

        }



        if score > 0 {

            results.push(
                SearchResult {

                    device:
                        device.clone(),

                    score,

                    reasons,

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


    if results.len() == 1 {

        return ResolveResult::Found {

            device:
                results[0].device.clone(),

            score:
                results[0].score,

            reasons:
                results[0].reasons.clone(),

        };

    }


    ResolveResult::Multiple {

        devices: results

    }

}








    pub fn log_all(
        &self
    ) {


        for device in self.entities.values() {


            debug(
                &format!(
                "{} = {}",
                device.entity_id,
                device.state
                )
            );


        }


    }

}
