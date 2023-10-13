use std::fs;

use async_graphql::Result;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use csv::ReaderBuilder;
use itertools::Itertools;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::fitness::{
    EntityAssets, SetLot, UserExerciseInput, UserWorkoutInput, UserWorkoutSetRecord,
    WorkoutSetStatistic,
};

use super::{DeployStrongAppImportInput, ImportResult};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
struct Entry {
    date: String,
    notes: Option<String>,
    weight: Option<Decimal>,
    reps: Option<usize>,
    distance: Option<Decimal>,
    seconds: Option<Decimal>,
    #[serde(alias = "Set Order")]
    set_order: u8,
    #[serde(alias = "Workout Duration")]
    workout_duration: String,
    #[serde(alias = "Workout Name")]
    workout_name: String,
    #[serde(alias = "Workout Notes")]
    workout_notes: Option<String>,
    #[serde(alias = "Exercise Name")]
    exercise_name: String,
}

pub async fn import(input: DeployStrongAppImportInput) -> Result<ImportResult> {
    fs::write(
        "tmp/strong_app_mappings.json",
        serde_json::to_string_pretty(&input.mapping).unwrap(),
    )
    .unwrap();
    let file_string = fs::read_to_string(&input.export_path)?;
    let mut workouts = vec![];
    let mut entries_reader = ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(file_string.as_bytes())
        .deserialize::<Entry>()
        .map(|r| r.unwrap())
        .collect_vec();
    // DEV: without this, the last workout does not get appended
    entries_reader.push(Entry {
        date: "invalid".to_string(),
        set_order: 0,
        ..Default::default()
    });
    let mut exercises = vec![];
    let mut sets = vec![];
    for (entry, next_entry) in entries_reader.into_iter().tuple_windows() {
        sets.push(UserWorkoutSetRecord {
            statistic: WorkoutSetStatistic {
                duration: entry.seconds,
                distance: entry.distance,
                reps: entry.reps,
                weight: entry.weight,
            },
            lot: SetLot::Normal,
        });
        if next_entry.set_order <= entry.set_order {
            exercises.push(UserExerciseInput {
                exercise_id: input
                    .mapping
                    .iter()
                    .find(|m| m.source_name == entry.exercise_name)
                    .unwrap()
                    .target_id,
                sets,
                notes: Vec::from_iter(entry.notes.clone()),
                rest_time: None,
                assets: EntityAssets::default(),
            });
            sets = vec![];
        }
        if next_entry.date != entry.date {
            let ndt = NaiveDateTime::parse_from_str(&entry.date, "%Y-%m-%d %H:%M:%S")
                .expect("Failed to parse input string");
            let ndt = DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc);
            let parts: Vec<&str> = entry.workout_duration.split_whitespace().collect();
            let hours = parts[0].trim_matches('h').parse::<i64>().unwrap_or(0);
            let minutes = parts[1].trim_matches('m').parse::<i64>().unwrap_or(0);
            let workout_duration = Duration::hours(hours) + Duration::minutes(minutes);
            workouts.push(UserWorkoutInput {
                name: entry.workout_name,
                comment: entry.workout_notes,
                start_time: ndt,
                end_time: ndt + workout_duration,
                exercises,
                supersets: vec![],
                assets: EntityAssets::default(),
            });
            exercises = vec![];
        }
    }
    Ok(ImportResult {
        collections: vec![],
        media: vec![],
        failed_items: vec![],
        workouts,
    })
}
