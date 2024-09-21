use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use pony::fs::find_files_in_dir;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
struct Exchangetitle {
	title: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Exchange {
	title: String,
	id: i32,
	token: String,
	stage: Stage,
	submissions: HashMap<String, Vec<Vec<String>>>,
	votes: HashMap<String, Vec<Vec<String>>>,
	results: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Stage {
	Submission,
	Voting,
	Selection,
}

#[post("/create-exchange")]
async fn create_exchange(item: web::Query<Exchangetitle>) -> impl Responder {
	let new_item = item.into_inner();
	println!("{:#?}", new_item);
	HttpResponse::Ok().json(new_item)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let exchanges = Arc::new(Mutex::new(HashMap::<i32, Exchange>::new()));
	if !Path::new("./exchanges").exists() {
		fs::create_dir("./exchanges")?
	}
	let ext = Regex::new(r".*\.json$").unwrap();
	for file in find_files_in_dir("./exchanges", false)
		.unwrap()
		.iter()
		.filter(|file| ext.is_match(file))
	{
		let text = fs::read_to_string(file)?;
		let exchange: Exchange = serde_json::from_str(&text)?;
		exchanges.lock().unwrap().insert(exchange.id, exchange);
	}

	HttpServer::new(move || {
		App::new()
			.app_data(web::Data::new(exchanges.clone()))
			.service(create_exchange)
	})
	//                  pony
	.bind(("127.0.0.1", 7669))?
	.run()
	.await
}
