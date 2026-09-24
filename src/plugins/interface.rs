use anyhow::Result;
use async_trait::async_trait;


#[derive(Debug)]
pub struct PluginResponse {

    pub success: bool,

    pub message: String,

}



#[async_trait]
pub trait Plugin: Send + Sync {


    fn name(
        &self
    ) -> &str;



    fn description(
        &self
    ) -> &str;

       async fn start(
        &mut self
    ) -> Result<()>;




    async fn execute(
        &mut self,
        command: String,
    ) -> Result<PluginResponse>;

}