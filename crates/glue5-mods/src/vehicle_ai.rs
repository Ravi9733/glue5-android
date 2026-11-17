//! Offline AI-assist foundation for Vehicle Hub.
//!
//! This is a small, deterministic on-device assistant: it interprets common
//! car-search intents, ranks catalog entries and explains install readiness.
//! It does not pretend to be a hosted generative model and never marks an
//! unsupported asset format as installable or spawnable.

use crate::vehicle_hub::{Catalog, PackageFormat, VehiclePackage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchIntent {
    Brand,
    Performance,
    Drift,
    OffRoad,
    Any,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VehicleSuggestion {
    pub id: String,
    pub score: u16,
    pub reason: String,
    pub install_note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantReply {
    pub intent: SearchIntent,
    pub summary: String,
    pub suggestions: Vec<VehicleSuggestion>,
}

/// Interpret a short natural-language request and rank only catalog entries.
/// Search is case-insensitive and works offline after the catalog is cached.
pub fn recommend(catalog: &Catalog, request: &str, limit: usize) -> AssistantReply {
    let query = request.to_ascii_lowercase();
    let intent = detect_intent(&query);
    let words: Vec<&str> = query.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() > 1).collect();

    let mut ranked: Vec<VehicleSuggestion> = catalog.vehicles.iter()
        .map(|vehicle| suggestion(vehicle, &words, intent))
        .filter(|item| item.score > 0)
        .collect();
    ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    ranked.truncate(limit.min(20));

    let summary = if ranked.is_empty() {
        "Catalog mein is request ka exact match nahi mila. Brand ya car model ka naam try karo.".to_string()
    } else {
        format!("{} matching car option(s) mile. Install/spawn support status har result ke saath diya hai.", ranked.len())
    };
    AssistantReply { intent, summary, suggestions: ranked }
}

fn detect_intent(query: &str) -> SearchIntent {
    if ["drift", "drifting", "slide"].iter().any(|w| query.contains(w)) {
        SearchIntent::Drift
    } else if ["offroad", "off-road", "4x4", "dirt", "rally"].iter().any(|w| query.contains(w)) {
        SearchIntent::OffRoad
    } else if ["fast", "speed", "supercar", "hypercar", "racing"].iter().any(|w| query.contains(w)) {
        SearchIntent::Performance
    } else if ["lamborghini", "ferrari", "bmw", "mercedes", "bugatti", "porsche", "audi", "nissan", "mazda"].iter().any(|w| query.contains(w)) {
        SearchIntent::Brand
    } else {
        SearchIntent::Any
    }
}

fn suggestion(vehicle: &VehiclePackage, words: &[&str], intent: SearchIntent) -> VehicleSuggestion {
    let haystack = format!("{} {} {}", vehicle.id, vehicle.name, vehicle.author).to_ascii_lowercase();
    let mut score = 0u16;
    for word in words {
        if haystack.split(|c: char| !c.is_ascii_alphanumeric()).any(|part| part == *word) {
            score = score.saturating_add(if vehicle.name.to_ascii_lowercase().contains(word) { 12 } else { 6 });
        }
        if vehicle.tags.iter().any(|tag| tag == word) {
            score = score.saturating_add(18);
        }
    }
    let wanted_tag = match intent {
        SearchIntent::Performance => Some(["supercar", "hypercar", "performance", "racing"].as_slice()),
        SearchIntent::Drift => Some(["drift", "drifting"].as_slice()),
        SearchIntent::OffRoad => Some(["offroad", "rally", "4x4"].as_slice()),
        _ => None,
    };
    if let Some(tags) = wanted_tag {
        if vehicle.tags.iter().any(|tag| tags.contains(&tag.as_str())) {
            score = score.saturating_add(24);
        }
    }
    // The initial catalog schema has no verified vehicle-class metadata.
    // Do not infer handling/performance from a name: explain this limitation.
    if intent == SearchIntent::Any && words.is_empty() {
        score = 1;
    }
    let install_note = match (vehicle.format, vehicle.spawn_supported) {
        (_, true) => "Native loader reports this package as spawn-supported; verify package signature before install.".to_string(),
        (PackageFormat::RpfDlc, false) => "RPF/DLC detected, but native third-party DLC mounting is not enabled yet; download/spawn disabled.".to_string(),
        (PackageFormat::LooseFiles, false) => "Loose files detected, but native vehicle registration is not enabled yet; download/spawn disabled.".to_string(),
    };
    let reason = if score > 0 {
        "Name/brand text matched your request.".to_string()
    } else {
        "No direct text match.".to_string()
    };
    VehicleSuggestion { id: vehicle.id.clone(), score, reason, install_note }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle_hub::Catalog;

    #[test]
    fn understands_brand_search_and_never_claims_rpf_ready() {
        let json = br#"{"version":1,"vehicles":[{"id":"mazda-furai","name":"Mazda Furai","version":"1","author":"Community","download_url":"https://example.com/furai.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size_bytes":100,"format":"rpf_dlc","tags":["supercar","drift"]}]}"#;
        let catalog = Catalog::parse(json).unwrap();
        let reply = recommend(&catalog, "Mazda Furai latest", 5);
        assert_eq!(reply.intent, SearchIntent::Brand);
        assert_eq!(reply.suggestions[0].id, "mazda-furai");
        assert!(reply.suggestions[0].install_note.contains("not enabled"));
        assert!(reply.suggestions[0].score >= 18);
    }

    #[test]
    fn performance_intent_is_detected_without_making_fake_vehicle_claims() {
        let json = br#"{"version":1,"vehicles":[]}"#;
        let catalog = Catalog::parse(json).unwrap();
        assert_eq!(recommend(&catalog, "fast supercar", 5).intent, SearchIntent::Performance);
    }
}
