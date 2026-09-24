use std::sync::Arc;

use tokio::sync::Mutex;

use anyhow::Result;

use crate::ha::client::HAClient;
use crate::core::entity_cache::EntityCache;


pub struct AstraHome {

    client:
        Arc<Mutex<HAClient>>,


}


impl AstraHome {

pub fn client(
    &self
) -> Arc<Mutex<HAClient>> {

    self.client.clone()

}
    pub fn new(
        client: Arc<Mutex<HAClient>>,

    ) -> Self {

        Self {

            client,


        }

    }


    pub async fn start(
        &self
    )
    -> Result<()> {


        let client =
            self.client.clone();


        tokio::spawn(
            async move {


                let mut client =
                    client.lock()
                    .await;


                if let Err(e) =
                    client.run().await
                {

                    crate::logger::error(
                        &format!(
                            "HA CLIENT STOPPED: {}",
                            e
                        )
                    );

                }


            }
        );


        Ok(())

    }



  /*   pub async fn cache_size(
        &self
    )
    -> usize {


        let cache =
            self.cache.lock()
            .await;


        cache.count()


    }
*/

}