//! Asset classification tools.
//!
//! These tools prepare asset taxonomy assignment previews for the chat widget.
//! They never write to the database; the frontend applies accepted drafts with
//! existing taxonomy assignment mutations after user confirmation.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use wealthfolio_core::{
    assets::{parse_symbol_with_exchange_suffix, Asset},
    taxonomies::{AssetTaxonomyAssignment, Category, TaxonomyWithCategories},
};

use crate::env::AiEnvironment;
use crate::error::AiError;

const ASSET_SCOPE: &str = "asset";
const AI_ASSIGNMENT_SOURCE: &str = "ai";

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssetTaxonomiesArgs {
    pub taxonomy_id: Option<String>,
    pub taxonomy_name: Option<String>,
    pub include_categories: Option<bool>,
    pub category_depth: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAssetTaxonomyAssignmentsArgs {
    pub asset_query: String,
    pub taxonomy_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAssetClassificationArgs {
    pub asset_query: String,
    pub taxonomy_id: String,
    pub assignments: Vec<PreparedAssignmentInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedAssignmentInput {
    pub category_id: String,
    pub weight_basis_points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAssetDto {
    pub asset_id: String,
    pub label: String,
    pub display_code: Option<String>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub exchange_mic: Option<String>,
    pub currency: String,
    pub matched_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTaxonomyCategoryDto {
    pub category_id: String,
    pub taxonomy_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub key: String,
    pub color: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTaxonomyDto {
    pub taxonomy_id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub is_single_select: bool,
    pub sort_order: i32,
    pub category_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<AssetTaxonomyCategoryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssetTaxonomiesOutput {
    pub taxonomies: Vec<AssetTaxonomyDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTaxonomyAssignmentDto {
    pub assignment_id: String,
    pub taxonomy_id: String,
    pub taxonomy_name: String,
    pub category_id: String,
    pub category_name: String,
    pub category_key: String,
    pub weight_basis_points: i32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAssetTaxonomyAssignmentsOutput {
    pub asset_query: String,
    pub resolved_asset: ResolvedAssetDto,
    pub assignments: Vec<AssetTaxonomyAssignmentDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTaxonomyDto {
    pub taxonomy_id: String,
    pub name: String,
    pub is_single_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentPreviewDto {
    pub assignment_id: Option<String>,
    pub category_id: String,
    pub category_name: String,
    pub category_key: String,
    pub category_color: String,
    pub weight_basis_points: i32,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationChangesDto {
    pub add_count: usize,
    pub update_count: usize,
    pub remove_count: usize,
    pub unchanged_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateAssignmentPreviewDto {
    pub asset_id: String,
    pub current_assignments: Vec<AssignmentPreviewDto>,
    pub changes: ClassificationChangesDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAssetClassificationOutput {
    pub asset_query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_asset: Option<ResolvedAssetDto>,
    pub taxonomy: PreparedTaxonomyDto,
    pub current_assignments: Vec<AssignmentPreviewDto>,
    pub proposed_assignments: Vec<AssignmentPreviewDto>,
    pub changes: ClassificationChangesDto,
    pub unallocated_basis_points: i32,
    pub draft_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_candidates: Vec<ResolvedAssetDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_current_assignments: Vec<CandidateAssignmentPreviewDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<String>,
}

pub struct ListAssetTaxonomiesTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> ListAssetTaxonomiesTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for ListAssetTaxonomiesTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for ListAssetTaxonomiesTool<E> {
    const NAME: &'static str = "list_asset_taxonomies";

    type Error = AiError;
    type Args = ListAssetTaxonomiesArgs;
    type Output = ListAssetTaxonomiesOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List asset-scoped taxonomy summaries, or categories for one selected \
                 asset taxonomy. First call without arguments to choose the taxonomy. Then call \
                 with taxonomyId or taxonomyName and includeCategories=true to get category IDs. \
                 For sector/top-level allocation requests, use categoryDepth=\"root\" so only \
                 root categories are returned. For region screenshots that list countries, use \
                 categoryDepth=\"all\" to get child country categories and parent IDs; use \
                 matching leaf country category IDs when available, and aggregate to root region \
                 categories only for top-level/root region requests. Use categoryDepth=\"all\" \
                 for detailed industry/subindustry requests."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "taxonomyId": {
                        "type": "string",
                        "description": "Optional asset-scoped taxonomy ID. Use this when you already selected the taxonomy from a prior summary call."
                    },
                    "taxonomyName": {
                        "type": "string",
                        "description": "Optional exact taxonomy name to fetch. Prefer taxonomyId when available."
                    },
                    "includeCategories": {
                        "type": "boolean",
                        "description": "Whether to include category IDs. Defaults to false for summary calls and true when taxonomyId or taxonomyName is provided."
                    },
                    "categoryDepth": {
                        "type": "string",
                        "enum": ["root", "all"],
                        "description": "Category set to return when includeCategories is true. Defaults to root. Use root for sector/top-level allocation; use all only for detailed categories."
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let taxonomies = asset_taxonomies(&self.env)?;
        let taxonomy_id = args
            .taxonomy_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let taxonomy_name = args
            .taxonomy_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let filters_taxonomy = taxonomy_id.is_some() || taxonomy_name.is_some();
        let include_categories = args.include_categories.unwrap_or(filters_taxonomy);
        let category_depth = parse_category_depth(args.category_depth.as_deref())?;
        let taxonomies = filter_asset_taxonomies(&taxonomies, taxonomy_id, taxonomy_name)?;

        Ok(ListAssetTaxonomiesOutput {
            taxonomies: taxonomies
                .iter()
                .map(|entry| to_asset_taxonomy_dto(entry, include_categories, category_depth))
                .collect(),
        })
    }
}

pub struct GetAssetTaxonomyAssignmentsTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> GetAssetTaxonomyAssignmentsTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for GetAssetTaxonomyAssignmentsTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for GetAssetTaxonomyAssignmentsTool<E> {
    const NAME: &'static str = "get_asset_taxonomy_assignments";

    type Error = AiError;
    type Args = GetAssetTaxonomyAssignmentsArgs;
    type Output = GetAssetTaxonomyAssignmentsOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read current asset taxonomy assignments for one active local asset. \
                 assetQuery may be an asset ID, exact ticker/display code, provider-suffixed \
                 ticker like SHOP.TO, or an asset name such as Apple Inc. Use this for \
                 read-only current-classification questions. For classification update/draft \
                 requests, use prepare_asset_classification instead because it returns current \
                 assignments and handles ambiguous asset matches in the widget."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetQuery": {
                        "type": "string",
                        "description": "Active local asset ID, ticker/display code, provider-suffixed ticker, or asset name."
                    },
                    "taxonomyId": {
                        "type": "string",
                        "description": "Optional taxonomy ID filter. Omit to return all asset-scoped taxonomy assignments for the asset."
                    }
                },
                "required": ["assetQuery"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let taxonomies = asset_taxonomies(&self.env)?;
        let taxonomy_lookup = taxonomy_lookup(&taxonomies);
        if let Some(taxonomy_id) = args.taxonomy_id.as_deref() {
            validate_asset_taxonomy(&taxonomies, taxonomy_id)?;
        }

        let asset = resolve_active_asset(&self.env, &args.asset_query)?;
        let assignments = self
            .env
            .taxonomy_service()
            .get_asset_assignments(&asset.asset.id)?
            .into_iter()
            .filter(|assignment| {
                args.taxonomy_id
                    .as_ref()
                    .is_none_or(|taxonomy_id| &assignment.taxonomy_id == taxonomy_id)
            })
            .filter_map(|assignment| assignment_dto(&assignment, &taxonomy_lookup))
            .collect();

        Ok(GetAssetTaxonomyAssignmentsOutput {
            asset_query: args.asset_query,
            resolved_asset: asset.to_dto(),
            assignments,
        })
    }
}

pub struct PrepareAssetClassificationTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> PrepareAssetClassificationTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for PrepareAssetClassificationTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for PrepareAssetClassificationTool<E> {
    const NAME: &'static str = "prepare_asset_classification";

    type Error = AiError;
    type Args = PrepareAssetClassificationArgs;
    type Output = PrepareAssetClassificationOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Prepare a non-mutating asset classification draft for the review widget. \
                 Use category IDs from list_asset_taxonomies for the selected taxonomy only. \
                 For sector allocation requests, use root/top-level categories from \
                 list_asset_taxonomies instead of detailed industry or subindustry categories. \
                 For region allocation requests based on country rows, use leaf country categories \
                 when that is the requested granularity; aggregate to root regions only for \
                 top-level/root region requests. \
                 Omit screenshot buckets such as Unknown, Other, Unclassified, or N/A when they \
                 do not exactly match an available category; never invent placeholder category IDs. \
                 This tool does not apply changes; the user must confirm the widget."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "assetQuery": {
                        "type": "string",
                        "description": "Active local asset ID, ticker/display code, provider-suffixed ticker, or asset name."
                    },
                    "taxonomyId": {
                        "type": "string",
                        "description": "Asset-scoped taxonomy ID from list_asset_taxonomies."
                    },
                    "assignments": {
                        "type": "array",
                        "description": "Proposed categories for this asset and taxonomy. Empty array clears current assignments for the taxonomy.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "categoryId": { "type": "string" },
                                "weightBasisPoints": {
                                    "type": "integer",
                                    "minimum": 0,
                                    "maximum": 10000
                                }
                            },
                            "required": ["categoryId", "weightBasisPoints"]
                        }
                    }
                },
                "required": ["assetQuery", "taxonomyId", "assignments"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let taxonomies = asset_taxonomies(&self.env)?;
        let taxonomy = validate_asset_taxonomy(&taxonomies, &args.taxonomy_id)?;
        let category_lookup: HashMap<&str, &Category> = taxonomy
            .categories
            .iter()
            .map(|category| (category.id.as_str(), category))
            .collect();

        validate_proposed_assignments(
            taxonomy.taxonomy.is_single_select,
            &category_lookup,
            &args.assignments,
        )?;

        let proposed_assignments = args
            .assignments
            .iter()
            .filter(|assignment| assignment.weight_basis_points > 0)
            .map(|assignment| proposed_preview_dto(assignment, &category_lookup))
            .collect::<Result<Vec<_>, _>>()?;

        let total_weight: i32 = proposed_assignments
            .iter()
            .map(|assignment| assignment.weight_basis_points)
            .sum();

        let asset = match resolve_active_asset_match(&self.env, &args.asset_query)? {
            ActiveAssetResolution::Resolved(asset) => asset,
            ActiveAssetResolution::Ambiguous(candidates) => {
                let candidate_current_assignments = candidates
                    .iter()
                    .map(|asset| {
                        let current_assignments = current_assignments_for_asset(
                            &self.env,
                            asset.id.as_str(),
                            &args.taxonomy_id,
                            &category_lookup,
                        )?;
                        Ok(CandidateAssignmentPreviewDto {
                            asset_id: asset.id.clone(),
                            changes: compute_changes(&current_assignments, &proposed_assignments),
                            current_assignments,
                        })
                    })
                    .collect::<Result<Vec<_>, AiError>>()?;

                return Ok(PrepareAssetClassificationOutput {
                    asset_query: args.asset_query,
                    resolved_asset: None,
                    taxonomy: PreparedTaxonomyDto {
                        taxonomy_id: taxonomy.taxonomy.id.clone(),
                        name: taxonomy.taxonomy.name.clone(),
                        is_single_select: taxonomy.taxonomy.is_single_select,
                    },
                    changes: ClassificationChangesDto::default(),
                    current_assignments: Vec::new(),
                    proposed_assignments,
                    unallocated_basis_points: 10000 - total_weight,
                    draft_status: "needsAssetSelection".to_string(),
                    asset_candidates: candidates
                        .iter()
                        .map(|asset| asset_to_dto(asset, "candidate"))
                        .collect(),
                    candidate_current_assignments,
                    applied_at: None,
                });
            }
            ActiveAssetResolution::NotFound(query) => {
                return Err(AiError::invalid_input(format!(
                    "Asset '{query}' was not found among active assets"
                )));
            }
        };

        let current_assignments = current_assignments_for_asset(
            &self.env,
            &asset.asset.id,
            &args.taxonomy_id,
            &category_lookup,
        )?;

        Ok(PrepareAssetClassificationOutput {
            asset_query: args.asset_query,
            resolved_asset: Some(asset.to_dto()),
            taxonomy: PreparedTaxonomyDto {
                taxonomy_id: taxonomy.taxonomy.id.clone(),
                name: taxonomy.taxonomy.name.clone(),
                is_single_select: taxonomy.taxonomy.is_single_select,
            },
            changes: compute_changes(&current_assignments, &proposed_assignments),
            current_assignments,
            proposed_assignments,
            unallocated_basis_points: 10000 - total_weight,
            draft_status: "draft".to_string(),
            asset_candidates: Vec::new(),
            candidate_current_assignments: Vec::new(),
            applied_at: None,
        })
    }
}

struct ResolvedAsset {
    asset: Asset,
    matched_by: &'static str,
}

impl ResolvedAsset {
    fn to_dto(&self) -> ResolvedAssetDto {
        asset_to_dto(&self.asset, self.matched_by)
    }
}

enum ActiveAssetResolution {
    Resolved(Box<ResolvedAsset>),
    Ambiguous(Vec<Asset>),
    NotFound(String),
}

enum UniqueAssetMatch {
    None,
    One(Box<Asset>),
    Ambiguous(Vec<Asset>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CategoryDepth {
    Root,
    All,
}

fn resolve_active_asset<E: AiEnvironment>(
    env: &Arc<E>,
    asset_query: &str,
) -> Result<ResolvedAsset, AiError> {
    match resolve_active_asset_match(env, asset_query)? {
        ActiveAssetResolution::Resolved(asset) => Ok(*asset),
        ActiveAssetResolution::Ambiguous(candidates) => Err(ambiguous_asset_error(&candidates)),
        ActiveAssetResolution::NotFound(query) => Err(AiError::invalid_input(format!(
            "Asset '{query}' was not found among active assets"
        ))),
    }
}

fn resolve_active_asset_match<E: AiEnvironment>(
    env: &Arc<E>,
    asset_query: &str,
) -> Result<ActiveAssetResolution, AiError> {
    let query = asset_query.trim();
    if query.is_empty() {
        return Err(AiError::invalid_input("assetQuery is required"));
    }

    let active_assets = env
        .asset_service()
        .get_assets()?
        .into_iter()
        .filter(|asset| asset.is_active)
        .collect::<Vec<_>>();
    if active_assets.is_empty() {
        return Ok(ActiveAssetResolution::NotFound(query.to_string()));
    }

    match unique_by_result(&active_assets, |asset| asset.id.eq_ignore_ascii_case(query)) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "asset_id",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    match unique_by_result(&active_assets, |asset| {
        option_eq_ignore_ascii_case(asset.display_code.as_deref(), query)
            || option_eq_ignore_ascii_case(asset.instrument_symbol.as_deref(), query)
    }) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "symbol",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    let (base_symbol, exchange_mic) = parse_symbol_with_exchange_suffix(query);
    if let Some(exchange_mic) = exchange_mic {
        match unique_by_result(&active_assets, |asset| {
            (option_eq_ignore_ascii_case(asset.display_code.as_deref(), base_symbol)
                || option_eq_ignore_ascii_case(asset.instrument_symbol.as_deref(), base_symbol))
                && option_eq_ignore_ascii_case(
                    asset.instrument_exchange_mic.as_deref(),
                    exchange_mic,
                )
        }) {
            UniqueAssetMatch::One(asset) => {
                return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                    asset: *asset,
                    matched_by: "provider_suffix",
                })));
            }
            UniqueAssetMatch::Ambiguous(candidates) => {
                return Ok(ActiveAssetResolution::Ambiguous(candidates));
            }
            UniqueAssetMatch::None => {}
        }
    }

    match resolve_symbol_with_exchange_mic(&active_assets, query) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "symbol_exchange_mic",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    let normalized_query = normalize_lookup(query);
    match unique_by_result(&active_assets, |asset| {
        asset.name.as_deref().is_some_and(|name| {
            let normalized_name = normalize_lookup(name);
            normalized_name == normalized_query
        })
    }) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "name",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    match unique_by_result(&active_assets, |asset| {
        normalize_lookup(&asset_label(asset)) == normalized_query
            || normalize_lookup(&asset_candidate_label(asset)) == normalized_query
    }) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "label",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    match unique_by_result(&active_assets, |asset| {
        asset.name.as_deref().is_some_and(|name| {
            let normalized_name = normalize_lookup(name);
            normalized_name.starts_with(&normalized_query)
                || normalized_name.contains(&normalized_query)
        })
    }) {
        UniqueAssetMatch::One(asset) => {
            return Ok(ActiveAssetResolution::Resolved(Box::new(ResolvedAsset {
                asset: *asset,
                matched_by: "name_fuzzy",
            })));
        }
        UniqueAssetMatch::Ambiguous(candidates) => {
            return Ok(ActiveAssetResolution::Ambiguous(candidates));
        }
        UniqueAssetMatch::None => {}
    }

    Ok(ActiveAssetResolution::NotFound(query.to_string()))
}

fn unique_by_result(assets: &[Asset], matches: impl Fn(&Asset) -> bool) -> UniqueAssetMatch {
    let matched = assets
        .iter()
        .filter(|asset| matches(asset))
        .collect::<Vec<_>>();
    match matched.len() {
        0 => UniqueAssetMatch::None,
        1 => UniqueAssetMatch::One(Box::new((*matched[0]).clone())),
        _ => UniqueAssetMatch::Ambiguous(
            matched
                .iter()
                .take(8)
                .map(|asset| (*asset).clone())
                .collect(),
        ),
    }
}

fn ambiguous_asset_error(candidates: &[Asset]) -> AiError {
    AiError::invalid_input(format!(
        "Asset query is ambiguous. Candidates: {}",
        candidates
            .iter()
            .take(8)
            .map(asset_candidate_label)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn option_eq_ignore_ascii_case(value: Option<&str>, query: &str) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case(query.trim()))
}

fn normalize_lookup(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn asset_label(asset: &Asset) -> String {
    let code = asset
        .display_code
        .as_deref()
        .or(asset.instrument_symbol.as_deref())
        .unwrap_or(asset.id.as_str());
    match asset.name.as_deref() {
        Some(name) if !name.trim().is_empty() => format!("{code} - {name}"),
        _ => code.to_string(),
    }
}

fn resolve_symbol_with_exchange_mic(assets: &[Asset], query: &str) -> UniqueAssetMatch {
    let normalized_query = normalize_lookup(query);
    unique_by_result(assets, |asset| {
        let Some(code) = asset
            .display_code
            .as_deref()
            .or(asset.instrument_symbol.as_deref())
        else {
            return false;
        };

        let mut candidates = Vec::new();
        if let Some(mic) = asset.instrument_exchange_mic.as_deref() {
            candidates.push(format!("{code} {mic}"));
            candidates.push(format!("{code} {mic} {}", asset.quote_ccy));
        }

        candidates
            .iter()
            .any(|candidate| normalize_lookup(candidate) == normalized_query)
    })
}

fn asset_to_dto(asset: &Asset, matched_by: &str) -> ResolvedAssetDto {
    ResolvedAssetDto {
        asset_id: asset.id.clone(),
        label: asset_label(asset),
        display_code: asset.display_code.clone(),
        symbol: asset.instrument_symbol.clone(),
        name: asset.name.clone(),
        exchange_mic: asset.instrument_exchange_mic.clone(),
        currency: asset.quote_ccy.clone(),
        matched_by: matched_by.to_string(),
    }
}

fn asset_candidate_label(asset: &Asset) -> String {
    let mut qualifiers = Vec::new();
    if let Some(exchange_mic) = asset
        .instrument_exchange_mic
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        qualifiers.push(format!("mic: {exchange_mic}"));
    }
    if !asset.quote_ccy.trim().is_empty() {
        qualifiers.push(format!("currency: {}", asset.quote_ccy));
    }
    qualifiers.push(format!("id: {}", asset.id));

    format!("{} ({})", asset_label(asset), qualifiers.join(", "))
}

fn asset_taxonomies<E: AiEnvironment>(
    env: &Arc<E>,
) -> Result<Vec<TaxonomyWithCategories>, AiError> {
    Ok(env
        .taxonomy_service()
        .get_taxonomies_with_categories()?
        .into_iter()
        .filter(|entry| entry.taxonomy.scope == ASSET_SCOPE)
        .collect())
}

fn validate_asset_taxonomy<'a>(
    taxonomies: &'a [TaxonomyWithCategories],
    taxonomy_id: &str,
) -> Result<&'a TaxonomyWithCategories, AiError> {
    taxonomies
        .iter()
        .find(|entry| entry.taxonomy.id == taxonomy_id)
        .ok_or_else(|| {
            AiError::invalid_input(format!(
                "Taxonomy '{taxonomy_id}' was not found or is not asset-scoped"
            ))
        })
}

fn parse_category_depth(value: Option<&str>) -> Result<CategoryDepth, AiError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(CategoryDepth::Root),
        Some(value) if value.eq_ignore_ascii_case("root") => Ok(CategoryDepth::Root),
        Some(value) if value.eq_ignore_ascii_case("all") => Ok(CategoryDepth::All),
        Some(value) => Err(AiError::invalid_input(format!(
            "categoryDepth must be 'root' or 'all', got '{value}'"
        ))),
    }
}

fn filter_asset_taxonomies<'a>(
    taxonomies: &'a [TaxonomyWithCategories],
    taxonomy_id: Option<&str>,
    taxonomy_name: Option<&str>,
) -> Result<Vec<&'a TaxonomyWithCategories>, AiError> {
    let filtered = taxonomies
        .iter()
        .filter(|entry| {
            taxonomy_id.is_none_or(|id| entry.taxonomy.id == id)
                && taxonomy_name.is_none_or(|name| entry.taxonomy.name.eq_ignore_ascii_case(name))
        })
        .collect::<Vec<_>>();

    if (taxonomy_id.is_some() || taxonomy_name.is_some()) && filtered.is_empty() {
        return Err(AiError::invalid_input(
            "No asset-scoped taxonomy matched the requested taxonomy filter",
        ));
    }

    if taxonomy_id.is_none() && taxonomy_name.is_some() && filtered.len() > 1 {
        return Err(AiError::invalid_input(
            "Taxonomy name matched multiple asset-scoped taxonomies; use taxonomyId",
        ));
    }

    Ok(filtered)
}

fn validate_proposed_assignments(
    is_single_select: bool,
    category_lookup: &HashMap<&str, &Category>,
    assignments: &[PreparedAssignmentInput],
) -> Result<(), AiError> {
    let mut seen = HashSet::new();
    for assignment in assignments {
        if !(0..=10000).contains(&assignment.weight_basis_points) {
            return Err(AiError::invalid_input(format!(
                "Weight for category '{}' must be between 0 and 10000 basis points",
                assignment.category_id
            )));
        }
        if assignment.weight_basis_points == 0 {
            continue;
        }
        if !category_lookup.contains_key(assignment.category_id.as_str()) {
            return Err(AiError::invalid_input(format!(
                "Category '{}' does not belong to the selected taxonomy",
                assignment.category_id
            )));
        }
        if !seen.insert(assignment.category_id.as_str()) {
            return Err(AiError::invalid_input(format!(
                "Duplicate category ID '{}'",
                assignment.category_id
            )));
        }
    }

    if is_single_select {
        let non_zero_assignments = assignments
            .iter()
            .filter(|assignment| assignment.weight_basis_points > 0)
            .collect::<Vec<_>>();
        if non_zero_assignments.len() > 1 {
            return Err(AiError::invalid_input(
                "Single-select taxonomies allow only one category",
            ));
        }
        if let Some(assignment) = non_zero_assignments.first() {
            if assignment.weight_basis_points != 10000 {
                return Err(AiError::invalid_input(
                    "Single-select taxonomies require 10000 basis points",
                ));
            }
        }
    }

    Ok(())
}

fn current_assignments_for_asset<E: AiEnvironment>(
    env: &Arc<E>,
    asset_id: &str,
    taxonomy_id: &str,
    category_lookup: &HashMap<&str, &Category>,
) -> Result<Vec<AssignmentPreviewDto>, AiError> {
    Ok(env
        .taxonomy_service()
        .get_asset_assignments(asset_id)?
        .into_iter()
        .filter(|assignment| assignment.taxonomy_id == taxonomy_id)
        .filter_map(|assignment| current_preview_dto(&assignment, category_lookup))
        .collect())
}

fn to_asset_taxonomy_dto(
    entry: &TaxonomyWithCategories,
    include_categories: bool,
    category_depth: CategoryDepth,
) -> AssetTaxonomyDto {
    let categories = if include_categories {
        entry
            .categories
            .iter()
            .filter(|category| {
                category_depth == CategoryDepth::All || category.parent_id.as_deref().is_none()
            })
            .map(to_category_dto)
            .collect()
    } else {
        Vec::new()
    };

    AssetTaxonomyDto {
        taxonomy_id: entry.taxonomy.id.clone(),
        name: entry.taxonomy.name.clone(),
        description: entry.taxonomy.description.clone(),
        color: entry.taxonomy.color.clone(),
        is_single_select: entry.taxonomy.is_single_select,
        sort_order: entry.taxonomy.sort_order,
        category_count: entry.categories.len(),
        categories,
    }
}

fn to_category_dto(category: &Category) -> AssetTaxonomyCategoryDto {
    AssetTaxonomyCategoryDto {
        category_id: category.id.clone(),
        taxonomy_id: category.taxonomy_id.clone(),
        parent_id: category.parent_id.clone(),
        name: category.name.clone(),
        key: category.key.clone(),
        color: category.color.clone(),
        sort_order: category.sort_order,
    }
}

fn taxonomy_lookup(
    taxonomies: &[TaxonomyWithCategories],
) -> HashMap<(String, String), (&TaxonomyWithCategories, &Category)> {
    let mut lookup = HashMap::new();
    for taxonomy in taxonomies {
        for category in &taxonomy.categories {
            lookup.insert(
                (taxonomy.taxonomy.id.clone(), category.id.clone()),
                (taxonomy, category),
            );
        }
    }
    lookup
}

fn assignment_dto(
    assignment: &AssetTaxonomyAssignment,
    lookup: &HashMap<(String, String), (&TaxonomyWithCategories, &Category)>,
) -> Option<AssetTaxonomyAssignmentDto> {
    let lookup_key = (
        assignment.taxonomy_id.clone(),
        assignment.category_id.clone(),
    );
    let (taxonomy, category) = lookup.get(&lookup_key)?;
    Some(AssetTaxonomyAssignmentDto {
        assignment_id: assignment.id.clone(),
        taxonomy_id: assignment.taxonomy_id.clone(),
        taxonomy_name: taxonomy.taxonomy.name.clone(),
        category_id: assignment.category_id.clone(),
        category_name: category.name.clone(),
        category_key: category.key.clone(),
        weight_basis_points: assignment.weight,
        source: assignment.source.clone(),
    })
}

fn current_preview_dto(
    assignment: &AssetTaxonomyAssignment,
    category_lookup: &HashMap<&str, &Category>,
) -> Option<AssignmentPreviewDto> {
    let category = category_lookup.get(assignment.category_id.as_str())?;
    Some(AssignmentPreviewDto {
        assignment_id: Some(assignment.id.clone()),
        category_id: assignment.category_id.clone(),
        category_name: category.name.clone(),
        category_key: category.key.clone(),
        category_color: category.color.clone(),
        weight_basis_points: assignment.weight,
        source: assignment.source.clone(),
    })
}

fn proposed_preview_dto(
    assignment: &PreparedAssignmentInput,
    category_lookup: &HashMap<&str, &Category>,
) -> Result<AssignmentPreviewDto, AiError> {
    let category = category_lookup
        .get(assignment.category_id.as_str())
        .ok_or_else(|| {
            AiError::invalid_input(format!(
                "Category '{}' does not belong to the selected taxonomy",
                assignment.category_id
            ))
        })?;
    Ok(AssignmentPreviewDto {
        assignment_id: None,
        category_id: assignment.category_id.clone(),
        category_name: category.name.clone(),
        category_key: category.key.clone(),
        category_color: category.color.clone(),
        weight_basis_points: assignment.weight_basis_points,
        source: AI_ASSIGNMENT_SOURCE.to_string(),
    })
}

fn compute_changes(
    current: &[AssignmentPreviewDto],
    proposed: &[AssignmentPreviewDto],
) -> ClassificationChangesDto {
    let current_by_category = current
        .iter()
        .map(|assignment| (assignment.category_id.as_str(), assignment))
        .collect::<HashMap<_, _>>();
    let proposed_by_category = proposed
        .iter()
        .map(|assignment| (assignment.category_id.as_str(), assignment))
        .collect::<HashMap<_, _>>();

    let mut changes = ClassificationChangesDto::default();
    for proposed_assignment in proposed {
        match current_by_category.get(proposed_assignment.category_id.as_str()) {
            None => changes.add_count += 1,
            Some(current_assignment)
                if current_assignment.weight_basis_points
                    != proposed_assignment.weight_basis_points
                    || current_assignment.source != AI_ASSIGNMENT_SOURCE =>
            {
                changes.update_count += 1;
            }
            Some(_) => changes.unchanged_count += 1,
        }
    }
    for current_assignment in current {
        if !proposed_by_category.contains_key(current_assignment.category_id.as_str()) {
            changes.remove_count += 1;
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use chrono::NaiveDateTime;
    use rig::tool::Tool;
    use wealthfolio_core::{
        assets::{AssetKind, InstrumentType, QuoteMode},
        taxonomies::{AssetTaxonomyAssignment, Category, Taxonomy},
    };

    use crate::env::test_env::{MockAssetService, MockEnvironment, MockTaxonomyService};

    fn env_with(
        assets: Vec<Asset>,
        taxonomies: Vec<TaxonomyWithCategories>,
        assignments: Vec<AssetTaxonomyAssignment>,
    ) -> Arc<MockEnvironment> {
        let mut env = MockEnvironment::new();
        env.asset_service = Arc::new(MockAssetService { assets });
        env.taxonomy_service = Arc::new(MockTaxonomyService {
            taxonomies,
            assignments,
        });
        Arc::new(env)
    }

    fn test_asset(
        id: &str,
        display_code: &str,
        symbol: &str,
        exchange_mic: Option<&str>,
        name: &str,
        is_active: bool,
    ) -> Asset {
        Asset {
            id: id.to_string(),
            kind: AssetKind::Investment,
            name: Some(name.to_string()),
            display_code: Some(display_code.to_string()),
            is_active,
            quote_mode: QuoteMode::Market,
            quote_ccy: "USD".to_string(),
            instrument_type: Some(InstrumentType::Equity),
            instrument_symbol: Some(symbol.to_string()),
            instrument_exchange_mic: exchange_mic.map(str::to_string),
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            ..Default::default()
        }
    }

    fn test_taxonomy(
        id: &str,
        scope: &str,
        is_single_select: bool,
        categories: Vec<Category>,
    ) -> TaxonomyWithCategories {
        TaxonomyWithCategories {
            taxonomy: Taxonomy {
                id: id.to_string(),
                name: "Asset Class".to_string(),
                color: "#2563eb".to_string(),
                description: None,
                is_system: false,
                is_single_select,
                sort_order: 1,
                created_at: NaiveDateTime::default(),
                updated_at: NaiveDateTime::default(),
                scope: scope.to_string(),
            },
            categories,
        }
    }

    fn test_category(taxonomy_id: &str, id: &str, name: &str) -> Category {
        Category {
            id: id.to_string(),
            taxonomy_id: taxonomy_id.to_string(),
            parent_id: None,
            name: name.to_string(),
            key: name.to_lowercase().replace(' ', "_"),
            color: "#64748b".to_string(),
            description: None,
            sort_order: 1,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            icon: None,
        }
    }

    fn test_child_category(taxonomy_id: &str, id: &str, parent_id: &str, name: &str) -> Category {
        Category {
            parent_id: Some(parent_id.to_string()),
            ..test_category(taxonomy_id, id, name)
        }
    }

    fn test_assignment(
        id: &str,
        asset_id: &str,
        taxonomy_id: &str,
        category_id: &str,
        weight: i32,
        source: &str,
    ) -> AssetTaxonomyAssignment {
        AssetTaxonomyAssignment {
            id: id.to_string(),
            asset_id: asset_id.to_string(),
            taxonomy_id: taxonomy_id.to_string(),
            category_id: category_id.to_string(),
            weight,
            source: source.to_string(),
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
        }
    }

    async fn resolve_query(asset_query: &str, assets: Vec<Asset>) -> Result<String, AiError> {
        let tool = GetAssetTaxonomyAssignmentsTool::new(env_with(assets, vec![], vec![]));
        tool.call(GetAssetTaxonomyAssignmentsArgs {
            asset_query: asset_query.to_string(),
            taxonomy_id: None,
        })
        .await
        .map(|output| output.resolved_asset.asset_id)
    }

    #[tokio::test]
    async fn list_asset_taxonomies_returns_asset_scope_summaries_by_default() {
        let env = env_with(
            vec![],
            vec![
                test_taxonomy(
                    "asset-tax",
                    "asset",
                    false,
                    vec![
                        test_category("asset-tax", "equity", "Equity"),
                        test_child_category("asset-tax", "equity-us", "equity", "US Equity"),
                    ],
                ),
                test_taxonomy(
                    "activity-tax",
                    "activity",
                    false,
                    vec![test_category("activity-tax", "food", "Food")],
                ),
            ],
            vec![],
        );

        let output = ListAssetTaxonomiesTool::new(env)
            .call(ListAssetTaxonomiesArgs::default())
            .await
            .unwrap();

        assert_eq!(output.taxonomies.len(), 1);
        assert_eq!(output.taxonomies[0].taxonomy_id, "asset-tax");
        assert_eq!(output.taxonomies[0].category_count, 2);
        assert!(output.taxonomies[0].categories.is_empty());
    }

    #[tokio::test]
    async fn list_asset_taxonomies_can_return_root_categories_for_selected_taxonomy() {
        let env = env_with(
            vec![],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![
                    test_category("asset-tax", "equity", "Equity"),
                    test_child_category("asset-tax", "equity-us", "equity", "US Equity"),
                    test_category("asset-tax", "cash", "Cash"),
                ],
            )],
            vec![],
        );

        let output = ListAssetTaxonomiesTool::new(env)
            .call(ListAssetTaxonomiesArgs {
                taxonomy_id: Some("asset-tax".to_string()),
                include_categories: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(output.taxonomies.len(), 1);
        assert_eq!(output.taxonomies[0].category_count, 3);
        assert_eq!(
            output.taxonomies[0]
                .categories
                .iter()
                .map(|category| category.category_id.as_str())
                .collect::<Vec<_>>(),
            vec!["equity", "cash"]
        );
    }

    #[tokio::test]
    async fn list_asset_taxonomies_can_return_all_categories_for_selected_taxonomy() {
        let env = env_with(
            vec![],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![
                    test_category("asset-tax", "equity", "Equity"),
                    test_child_category("asset-tax", "equity-us", "equity", "US Equity"),
                ],
            )],
            vec![],
        );

        let output = ListAssetTaxonomiesTool::new(env)
            .call(ListAssetTaxonomiesArgs {
                taxonomy_name: Some("Asset Class".to_string()),
                include_categories: Some(true),
                category_depth: Some("all".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(
            output.taxonomies[0]
                .categories
                .iter()
                .map(|category| category.category_id.as_str())
                .collect::<Vec<_>>(),
            vec!["equity", "equity-us"]
        );
    }

    #[tokio::test]
    async fn get_assignments_resolves_asset_and_enriches_rows() {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![test_category("asset-tax", "equity", "Equity")],
            )],
            vec![test_assignment(
                "assignment-1",
                "asset-aapl",
                "asset-tax",
                "equity",
                10000,
                "manual",
            )],
        );

        let output = GetAssetTaxonomyAssignmentsTool::new(env)
            .call(GetAssetTaxonomyAssignmentsArgs {
                asset_query: "Apple".to_string(),
                taxonomy_id: Some("asset-tax".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(output.resolved_asset.asset_id, "asset-aapl");
        assert_eq!(output.assignments.len(), 1);
        assert_eq!(output.assignments[0].category_name, "Equity");
    }

    #[tokio::test]
    async fn get_assignments_handles_duplicate_category_ids_across_taxonomies() {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![
                test_taxonomy(
                    "asset-tax",
                    "asset",
                    false,
                    vec![test_category("asset-tax", "shared", "Equity")],
                ),
                test_taxonomy(
                    "factor-tax",
                    "asset",
                    false,
                    vec![test_category("factor-tax", "shared", "Value")],
                ),
            ],
            vec![test_assignment(
                "assignment-1",
                "asset-aapl",
                "asset-tax",
                "shared",
                10000,
                "manual",
            )],
        );

        let output = GetAssetTaxonomyAssignmentsTool::new(env)
            .call(GetAssetTaxonomyAssignmentsArgs {
                asset_query: "AAPL".to_string(),
                taxonomy_id: Some("asset-tax".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(output.assignments.len(), 1);
        assert_eq!(output.assignments[0].taxonomy_id, "asset-tax");
        assert_eq!(output.assignments[0].category_name, "Equity");
    }

    #[tokio::test]
    async fn prepare_asset_classification_returns_draft_preview() {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![
                    test_category("asset-tax", "equity", "Equity"),
                    test_category("asset-tax", "cash", "Cash"),
                ],
            )],
            vec![test_assignment(
                "assignment-1",
                "asset-aapl",
                "asset-tax",
                "equity",
                10000,
                "manual",
            )],
        );

        let output = PrepareAssetClassificationTool::new(env)
            .call(PrepareAssetClassificationArgs {
                asset_query: "AAPL".to_string(),
                taxonomy_id: "asset-tax".to_string(),
                assignments: vec![
                    PreparedAssignmentInput {
                        category_id: "equity".to_string(),
                        weight_basis_points: 6000,
                    },
                    PreparedAssignmentInput {
                        category_id: "cash".to_string(),
                        weight_basis_points: 3000,
                    },
                ],
            })
            .await
            .unwrap();

        assert_eq!(output.draft_status, "draft");
        assert_eq!(
            output
                .resolved_asset
                .as_ref()
                .map(|asset| asset.asset_id.as_str()),
            Some("asset-aapl"),
        );
        assert_eq!(output.current_assignments.len(), 1);
        assert_eq!(output.proposed_assignments.len(), 2);
        assert_eq!(output.changes.add_count, 1);
        assert_eq!(output.changes.update_count, 1);
        assert_eq!(output.unallocated_basis_points, 1000);
    }

    #[tokio::test]
    async fn prepare_returns_asset_selection_candidates_when_query_is_ambiguous() {
        let env = env_with(
            vec![
                test_asset(
                    "asset-vt-xnas",
                    "VT",
                    "VT",
                    Some("XNAS"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
                test_asset(
                    "asset-vt-arcx",
                    "VT",
                    "VT",
                    Some("ARCX"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
            ],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![test_category("asset-tax", "equity", "Equity")],
            )],
            vec![test_assignment(
                "assignment-xnas",
                "asset-vt-xnas",
                "asset-tax",
                "equity",
                10000,
                "manual",
            )],
        );

        let output = PrepareAssetClassificationTool::new(env)
            .call(PrepareAssetClassificationArgs {
                asset_query: "VT".to_string(),
                taxonomy_id: "asset-tax".to_string(),
                assignments: vec![PreparedAssignmentInput {
                    category_id: "equity".to_string(),
                    weight_basis_points: 9000,
                }],
            })
            .await
            .unwrap();

        assert_eq!(output.draft_status, "needsAssetSelection");
        assert!(output.resolved_asset.is_none());
        assert_eq!(output.asset_candidates.len(), 2);
        assert_eq!(output.asset_candidates[0].asset_id, "asset-vt-xnas");
        assert_eq!(
            output.asset_candidates[0].exchange_mic.as_deref(),
            Some("XNAS")
        );
        assert_eq!(output.asset_candidates[1].asset_id, "asset-vt-arcx");
        assert_eq!(output.proposed_assignments.len(), 1);
        assert_eq!(output.candidate_current_assignments.len(), 2);
        assert_eq!(
            output.candidate_current_assignments[0].asset_id,
            "asset-vt-xnas"
        );
        assert_eq!(
            output.candidate_current_assignments[0]
                .current_assignments
                .len(),
            1
        );
        assert_eq!(
            output.candidate_current_assignments[0].changes.update_count,
            1
        );
        assert_eq!(
            output.candidate_current_assignments[1].asset_id,
            "asset-vt-arcx"
        );
        assert_eq!(
            output.candidate_current_assignments[1]
                .current_assignments
                .len(),
            0
        );
        assert_eq!(output.candidate_current_assignments[1].changes.add_count, 1);
        assert_eq!(output.unallocated_basis_points, 1000);
    }

    #[tokio::test]
    async fn prepare_counts_source_only_difference_as_update() {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![test_category("asset-tax", "equity", "Equity")],
            )],
            vec![test_assignment(
                "assignment-1",
                "asset-aapl",
                "asset-tax",
                "equity",
                10000,
                "manual",
            )],
        );

        let output = PrepareAssetClassificationTool::new(env)
            .call(PrepareAssetClassificationArgs {
                asset_query: "AAPL".to_string(),
                taxonomy_id: "asset-tax".to_string(),
                assignments: vec![PreparedAssignmentInput {
                    category_id: "equity".to_string(),
                    weight_basis_points: 10000,
                }],
            })
            .await
            .unwrap();

        assert_eq!(output.changes.update_count, 1);
        assert_eq!(output.changes.unchanged_count, 0);
    }

    #[tokio::test]
    async fn resolver_matches_asset_id() {
        let id = resolve_query(
            "asset-aapl",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-aapl");
    }

    #[tokio::test]
    async fn resolver_matches_exact_candidate_label() {
        let id = resolve_query(
            "VT - Vanguard Total World Stock Index Fund ETF Shares (mic: ARCX, currency: USD, id: asset-vt-world)",
            vec![
                test_asset(
                    "asset-vt-world",
                    "VT",
                    "VT",
                    Some("ARCX"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
                test_asset(
                    "asset-vt-vistra",
                    "VT",
                    "VT",
                    Some("XNYS"),
                    "Vistra Corp.",
                    true,
                ),
            ],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-vt-world");
    }

    #[tokio::test]
    async fn resolver_matches_symbol_with_xnas_mic() {
        let id = resolve_query(
            "VT XNAS",
            vec![
                test_asset(
                    "asset-vt-xnas",
                    "VT",
                    "VT",
                    Some("XNAS"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
                test_asset(
                    "asset-vt-arcx",
                    "VT",
                    "VT",
                    Some("ARCX"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
            ],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-vt-xnas");
    }

    #[tokio::test]
    async fn resolver_matches_symbol_with_exchange_mic() {
        let id = resolve_query(
            "VT ARCX",
            vec![
                test_asset(
                    "asset-vt-xnas",
                    "VT",
                    "VT",
                    Some("XNAS"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
                test_asset(
                    "asset-vt-arcx",
                    "VT",
                    "VT",
                    Some("ARCX"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
            ],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-vt-arcx");
    }

    #[tokio::test]
    async fn resolver_ambiguity_candidates_include_unique_identifiers() {
        let error = resolve_query(
            "VT",
            vec![
                test_asset(
                    "asset-vt-arcx",
                    "VT",
                    "VT",
                    Some("ARCX"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
                test_asset(
                    "asset-vt-xnys",
                    "VT",
                    "VT",
                    Some("XNYS"),
                    "Vanguard Total World Stock Index Fund ETF Shares",
                    true,
                ),
            ],
        )
        .await
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains(
            "VT - Vanguard Total World Stock Index Fund ETF Shares (mic: ARCX, currency: USD, id: asset-vt-arcx)"
        ));
        assert!(message.contains(
            "VT - Vanguard Total World Stock Index Fund ETF Shares (mic: XNYS, currency: USD, id: asset-vt-xnys)"
        ));
    }

    #[tokio::test]
    async fn resolver_matches_ticker() {
        let id = resolve_query(
            "AAPL",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-aapl");
    }

    #[tokio::test]
    async fn resolver_matches_provider_suffix() {
        let id = resolve_query(
            "SHOP.TO",
            vec![test_asset(
                "asset-shop",
                "SHOP",
                "SHOP",
                Some("XTSE"),
                "Shopify Inc.",
                true,
            )],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-shop");
    }

    #[tokio::test]
    async fn resolver_matches_exact_name() {
        let id = resolve_query(
            "Apple Inc.",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-aapl");
    }

    #[tokio::test]
    async fn resolver_matches_unambiguous_fuzzy_name() {
        let id = resolve_query(
            "Apple",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
        )
        .await
        .unwrap();

        assert_eq!(id, "asset-aapl");
    }

    #[tokio::test]
    async fn resolver_excludes_inactive_assets() {
        let error = resolve_query(
            "AAPL",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                false,
            )],
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("not found among active assets"));
    }

    #[tokio::test]
    async fn resolver_returns_ambiguity_with_candidate_labels() {
        let error = resolve_query(
            "Apple",
            vec![
                test_asset(
                    "asset-aapl",
                    "AAPL",
                    "AAPL",
                    Some("XNAS"),
                    "Apple Inc.",
                    true,
                ),
                test_asset(
                    "asset-aple",
                    "APLE",
                    "APLE",
                    Some("XNYS"),
                    "Apple Hospitality REIT",
                    true,
                ),
            ],
        )
        .await
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("ambiguous"));
        assert!(message.contains("AAPL - Apple Inc. (mic: XNAS, currency: USD, id: asset-aapl)"));
        assert!(message
            .contains("APLE - Apple Hospitality REIT (mic: XNYS, currency: USD, id: asset-aple)"));
    }

    #[tokio::test]
    async fn resolver_returns_not_found_for_missing_active_asset() {
        let error = resolve_query(
            "MSFT",
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("not found among active assets"));
    }

    #[tokio::test]
    async fn prepare_rejects_duplicate_categories() {
        let error = prepare_error(vec![
            PreparedAssignmentInput {
                category_id: "equity".to_string(),
                weight_basis_points: 5000,
            },
            PreparedAssignmentInput {
                category_id: "equity".to_string(),
                weight_basis_points: 5000,
            },
        ])
        .await;

        assert!(error.to_string().contains("Duplicate category ID"));
    }

    #[tokio::test]
    async fn prepare_rejects_single_select_multiple_categories() {
        let error = prepare_single_select_error(vec![
            PreparedAssignmentInput {
                category_id: "equity".to_string(),
                weight_basis_points: 5000,
            },
            PreparedAssignmentInput {
                category_id: "cash".to_string(),
                weight_basis_points: 5000,
            },
        ])
        .await;

        assert!(error.to_string().contains("allow only one category"));
    }

    #[tokio::test]
    async fn prepare_rejects_single_select_partial_weight() {
        let error = prepare_single_select_error(vec![PreparedAssignmentInput {
            category_id: "equity".to_string(),
            weight_basis_points: 5000,
        }])
        .await;

        assert!(error.to_string().contains("require 10000 basis points"));
    }

    #[tokio::test]
    async fn prepare_rejects_invalid_weights() {
        let error = prepare_error(vec![PreparedAssignmentInput {
            category_id: "equity".to_string(),
            weight_basis_points: -1,
        }])
        .await;

        assert!(error.to_string().contains("between 0 and 10000"));
    }

    #[tokio::test]
    async fn prepare_allows_over_allocation_as_invalid_draft() {
        let output = prepare_success(vec![
            PreparedAssignmentInput {
                category_id: "equity".to_string(),
                weight_basis_points: 7000,
            },
            PreparedAssignmentInput {
                category_id: "cash".to_string(),
                weight_basis_points: 4000,
            },
        ])
        .await;

        assert_eq!(output.unallocated_basis_points, -1000);
        assert_eq!(output.proposed_assignments.len(), 2);
    }

    #[tokio::test]
    async fn prepare_allows_under_allocation() {
        let output = prepare_success(vec![PreparedAssignmentInput {
            category_id: "equity".to_string(),
            weight_basis_points: 6000,
        }])
        .await;

        assert_eq!(output.unallocated_basis_points, 4000);
    }

    #[tokio::test]
    async fn prepare_treats_zero_weight_assignments_as_removals() {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                false,
                vec![
                    test_category("asset-tax", "equity", "Equity"),
                    test_category("asset-tax", "cash", "Cash"),
                ],
            )],
            vec![
                test_assignment(
                    "assignment-equity",
                    "asset-aapl",
                    "asset-tax",
                    "equity",
                    6000,
                    "ai",
                ),
                test_assignment(
                    "assignment-cash",
                    "asset-aapl",
                    "asset-tax",
                    "cash",
                    4000,
                    "manual",
                ),
            ],
        );

        let output = PrepareAssetClassificationTool::new(env)
            .call(PrepareAssetClassificationArgs {
                asset_query: "AAPL".to_string(),
                taxonomy_id: "asset-tax".to_string(),
                assignments: vec![
                    PreparedAssignmentInput {
                        category_id: "equity".to_string(),
                        weight_basis_points: 6000,
                    },
                    PreparedAssignmentInput {
                        category_id: "cash".to_string(),
                        weight_basis_points: 0,
                    },
                ],
            })
            .await
            .unwrap();

        assert_eq!(output.current_assignments.len(), 2);
        assert_eq!(output.proposed_assignments.len(), 1);
        assert_eq!(output.proposed_assignments[0].category_id, "equity");
        assert_eq!(output.changes.remove_count, 1);
        assert_eq!(output.changes.unchanged_count, 1);
        assert_eq!(output.changes.add_count, 0);
        assert_eq!(output.changes.update_count, 0);
        assert_eq!(output.unallocated_basis_points, 4000);
    }

    #[tokio::test]
    async fn prepare_ignores_zero_weight_unknown_category() {
        let output = prepare_success(vec![
            PreparedAssignmentInput {
                category_id: "UNKNOWN".to_string(),
                weight_basis_points: 0,
            },
            PreparedAssignmentInput {
                category_id: "equity".to_string(),
                weight_basis_points: 10000,
            },
        ])
        .await;

        assert_eq!(output.proposed_assignments.len(), 1);
        assert_eq!(output.proposed_assignments[0].category_id, "equity");
        assert_eq!(output.unallocated_basis_points, 0);
    }

    async fn prepare_success(
        assignments: Vec<PreparedAssignmentInput>,
    ) -> PrepareAssetClassificationOutput {
        prepare_with_single_select(assignments, false)
            .await
            .unwrap()
    }

    async fn prepare_error(assignments: Vec<PreparedAssignmentInput>) -> AiError {
        prepare_with_single_select(assignments, false)
            .await
            .unwrap_err()
    }

    async fn prepare_single_select_error(assignments: Vec<PreparedAssignmentInput>) -> AiError {
        prepare_with_single_select(assignments, true)
            .await
            .unwrap_err()
    }

    async fn prepare_with_single_select(
        assignments: Vec<PreparedAssignmentInput>,
        is_single_select: bool,
    ) -> Result<PrepareAssetClassificationOutput, AiError> {
        let env = env_with(
            vec![test_asset(
                "asset-aapl",
                "AAPL",
                "AAPL",
                Some("XNAS"),
                "Apple Inc.",
                true,
            )],
            vec![test_taxonomy(
                "asset-tax",
                "asset",
                is_single_select,
                vec![
                    test_category("asset-tax", "equity", "Equity"),
                    test_category("asset-tax", "cash", "Cash"),
                ],
            )],
            vec![],
        );

        PrepareAssetClassificationTool::new(env)
            .call(PrepareAssetClassificationArgs {
                asset_query: "AAPL".to_string(),
                taxonomy_id: "asset-tax".to_string(),
                assignments,
            })
            .await
    }
}
