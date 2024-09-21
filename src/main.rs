use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use pony::fs::find_files_in_dir;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
struct ExchangeTitle {
	title: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Exchange {
	title: String,
	id: i32,
	token: String,
	stage: Stage,
	submissions: HashMap<String, Vec<Vec<String>>>,
	votes: HashMap<String, Vec<Vec<String>>>,
	results: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum Stage {
	Submission,
	Voting,
	Selection,
}

#[post("/create-exchange")]
async fn create_exchange(
	title: web::Query<ExchangeTitle>, data: web::Data<Arc<Mutex<HashMap<i32, Exchange>>>>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let title = title.into_inner().title;
	let mut exchanges = data.lock().unwrap();
	let id = if exchanges.is_empty() {
		1
	} else {
		exchanges.clone().into_keys().max().unwrap() + 1
	};
	let exchange = Exchange {
		title: title.clone(),
		id,
		token: generate_passphrase(),
		stage: Stage::Submission,
		submissions: HashMap::new(),
		votes: HashMap::new(),
		results: Vec::new(),
	};
	exchanges.insert(id, exchange.clone());
	let path = format!("./exchanges/{id}.json");
	let contents = serde_json::to_string_pretty(&exchange)?;
	fs::write(path, contents)?;
	Ok(HttpResponse::Created().json(exchange))
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

fn generate_passphrase() -> String {
	let characters = [
		"TwilightSparkle",
		"Rarity",
		"PinkiePie",
		"Fluttershy",
		"RainbowDash",
		"Applejack",
		"MayorMare",
		"DoctorWhooves",
		"Derpy",
		"Coloratura",
		"DaringDo",
		"PhotoFinish",
		"FancyPants",
		"SapphireShores",
		"Spitfire",
		"Soarin",
		"SunnyStarscout",
		"IzzyMoonbow",
		"QueenChrysalis",
		"SilkRose",
	];
	let actions = [
		"Boops",
		"Kisses",
		"Likes",
		"Loves",
		"WantsToDate",
		"Hugs",
		"Holds",
		"TalksTo",
		"SingsTo",
		"AsksOut",
		"GivesABitTo",
		"HasCoffeeWith",
		"HasAVerySpecificQuestionFor",
		"IsNotTheSamePonyAs",
		"HasACrushOn",
		"IsMarrying",
		"TripsInFrontOf",
		"LooksAt",
		"HoldsHoovesWith",
	];

	let mut rng = rand::thread_rng();

	let char1 = characters[rng.gen_range(0..characters.len())];
	let char2 = characters[rng.gen_range(0..characters.len())];

	if char1 == char2 {
		format!("{char1}IsTheSamePonyAs{char2}")
	} else {
		let action = actions[rng.gen_range(0..actions.len())];
		format!("{char1}{action}{char2}")
	}
}
