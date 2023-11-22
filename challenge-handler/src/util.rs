use std::env::var;

pub fn call_prover(param: String, function: &str) -> Option<String> {
    let prover_rpc = var("PROVER_RPC").expect("Cannot detect PROVER_RPC env var");

    let client = reqwest::blocking::Client::new();
    let url = prover_rpc.to_owned() + function;
    let response = client
        .post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        )
        .body(param.clone())
        .send();
    let rt: Result<String, reqwest::Error> = match response {
        Ok(x) => x.text(),
        Err(e) => {
            log::error!("call prover error, param =  {:#?}, error = {:#?}", param, e);
            return None;
        }
    };

    let rt_text = match rt {
        Ok(x) => x,
        Err(e) => {
            log::error!("fetch prover res_txt error, param =  {:#?}, error = {:#?}", param, e);
            return None;
        }
    };

    Some(rt_text)
}
