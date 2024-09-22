use actix_web::{delete, patch, post, web, App, HttpResponse, HttpServer, Responder};
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

#[derive(Debug, Serialize, Deserialize)]
struct ExchangeStage {
	stage: Stage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Submission {
	name: String,
	stories: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Exchange {
	title: String,
	id: i32,
	passphrase: String,
	stage: Stage,
	submissions: HashMap<String, Vec<Entry>>,
	votes: HashMap<String, Vec<Vote>>,
	results: HashMap<String, Vec<Entry>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct Entry {
	stories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct Vote {
	entry: Entry,
	priority: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum Stage {
	Submission,
	Voting,
	Selection,
	Frozen,
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
		passphrase: generate_passphrase(),
		stage: Stage::Submission,
		submissions: HashMap::new(),
		votes: HashMap::new(),
		results: HashMap::new(),
	};
	exchanges.insert(id, exchange.clone());
	let path = format!("./exchanges/{id}.json");
	let contents = serde_json::to_string_pretty(&exchange)?;
	fs::write(path, contents)?;
	Ok(HttpResponse::Created().json(exchange))
}

#[patch("/change-state/{id}/{passphrase}")]
async fn change_stage(
	path: web::Path<(i32, String)>, stage: web::Query<ExchangeStage>,
	data: web::Data<Arc<Mutex<HashMap<i32, Exchange>>>>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let (id, passphrase) = path.into_inner();
	let stage = stage.into_inner().stage;
	let mut exchanges = data.lock().map_err(|_| "Failed to lock data")?;
	if let Some(ref mut exchange) = exchanges.get_mut(&id) {
		if exchange.passphrase != passphrase {
			return Ok(HttpResponse::Unauthorized().body("Invalid passphrase"));
		}
		if exchange.stage == stage {
			return Ok(HttpResponse::BadRequest().body("Stage is identical to request"));
		}

		match (exchange.stage, stage) {
			(Stage::Submission, Stage::Voting) => {
				if exchange.submissions.is_empty() {
					return Ok(HttpResponse::BadRequest().body("No submission to vote on"));
				}
			}
			(Stage::Voting, Stage::Submission) => exchange.votes = HashMap::new(),
			(Stage::Voting, Stage::Selection) => {
				if exchange.votes.is_empty() {
					return Ok(HttpResponse::BadRequest().body("No votes to count"));
				}
				// TODO: Add voting algorithm
			}
			(Stage::Selection, Stage::Voting) => exchange.results = HashMap::new(),
			(Stage::Selection, Stage::Frozen) => {} // Results are final
			(Stage::Frozen, _) => {
				return Ok(
					HttpResponse::Locked().body("This exchange is frozen and cannot be modified")
				)
			}
			(_, _) => return Ok(HttpResponse::BadRequest().body("Invalid stage transition")),
		}

		exchange.stage = stage;
		let path = format!("./exchanges/{id}.json");
		let contents = serde_json::to_string_pretty(&exchange)?;
		fs::write(path, contents)?;

		Ok(HttpResponse::Ok().body("Stage updated"))
	} else {
		Ok(HttpResponse::NotFound().body("Exchange not found"))
	}
}

#[post("/add-stories/{id}")]
async fn add_submission(
	path: web::Path<i32>, entry: web::Json<Submission>,
	data: web::Data<Arc<Mutex<HashMap<i32, Exchange>>>>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let id = path.into_inner();
	let submission = entry.into_inner();
	let mut exchanges = data.lock().map_err(|_| "Failed to lock data")?;
	if let Some(ref mut exchange) = exchanges.get_mut(&id) {
		if exchange.stage != Stage::Submission {
			return Ok(HttpResponse::BadRequest().body("Submission stage is over"));
		}
		let stories = submission
			.stories
			.iter()
			.map(|i| Entry {
				stories: i.to_vec(),
			})
			.collect::<Vec<_>>();
		if let Some(ref mut entries) = exchange.submissions.get_mut(&submission.name) {
			'outer: for set in stories.iter() {
				for entry in entries.iter() {
					if set == entry {
						continue 'outer;
					}
				}
				entries.push(set.clone());
			}
		} else {
			exchange.submissions.insert(submission.name, stories);
		}

		let path = format!("./exchanges/{id}.json");
		let contents = serde_json::to_string_pretty(&exchange)?;
		fs::write(path, contents)?;

		Ok(HttpResponse::Ok().body("Submission accepted"))
	} else {
		Ok(HttpResponse::NotFound().body("Exchange not found"))
	}
}

#[delete("/delete-exchange/{id}/{passphrase}")]
async fn delete_exchange(
	path: web::Path<(i32, String)>, data: web::Data<Arc<Mutex<HashMap<i32, Exchange>>>>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let (id, passphrase) = path.into_inner();
	let mut exchanges = data.lock().map_err(|_| "Failed to lock data")?;
	if let Some(exchange) = exchanges.get(&id) {
		if exchange.passphrase != passphrase {
			return Ok(HttpResponse::Unauthorized().body("Invalid passphrase"));
		}
		exchanges.remove(&id);
		let path = format!("./exchanges/{id}.json");
		fs::remove_file(path)?;

		Ok(HttpResponse::Ok().body("Exchange deleted"))
	} else {
		Ok(HttpResponse::NotFound().body("Exchange not found"))
	}
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
			.service(delete_exchange)
			.service(change_stage)
			.service(add_submission)
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
