//! Tools that use the browser's data: extraction, flows, history, bookmarks, downloads,
//! boards, files, the clipboard and notifications.

use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::automation::flows::{self, FlowStep};
use crate::automation::{clip, extract};
use crate::bookmarks::{self, BookmarkNode};
use crate::settings::{self, AutomationSettings};
use crate::{boards, downloads, files, history_index, ocr, system, tabs};

use super::{Ctx, Done, Kind, Tool, arg_bool, arg_list, arg_str, arg_u32, halt, object, step, tool};

const FLOW_TIMEOUT: Duration = Duration::from_secs(900);

pub fn tools(_auto: &AutomationSettings) -> Vec<Tool> {
    vec![
        tool(
            "extract",
            "Pull structured rows from the current page, such as products, results or a table, and save them as a dataset the user can export.",
            object(
                json!({
                    "instruction": { "type": "string", "description": "What rows and columns to pull, such as `each product's name, price and rating`." },
                    "name": { "type": "string", "description": "Dataset name." },
                    "save": { "type": "boolean", "description": "Save as a dataset. Defaults to true." }
                }),
                &["instruction"],
            ),
            Kind::Read,
            true,
            extract_rows,
        ),
        tool(
            "crawl",
            "Visit several addresses in the background and pull the same kind of rows from each into one dataset.",
            object(
                json!({
                    "urls": { "type": "array", "items": { "type": "string" } },
                    "instruction": { "type": "string" },
                    "name": { "type": "string" }
                }),
                &["urls", "instruction"],
            ),
            Kind::Meta,
            false,
            crawl,
        ),
        tool(
            "list_flows",
            "List the user's saved automation flows.",
            object(json!({}), &[]),
            Kind::Read,
            false,
            list_flows,
        ),
        tool(
            "run_flow",
            "Run one of the user's saved flows in the current tab and wait for it to finish.",
            object(
                json!({
                    "flowId": { "type": "string" },
                    "vars": { "type": "object", "description": "Values for the flow's {{variables}}.", "additionalProperties": { "type": "string" } }
                }),
                &["flowId"],
            ),
            Kind::Page,
            false,
            run_flow,
        ),
        tool(
            "search_history",
            "Search the user's browsing history by words in page titles, addresses and text.",
            object(
                json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
                &["query"],
            ),
            Kind::Read,
            false,
            search_history,
        ),
        tool(
            "bookmark_find",
            "Search the user's bookmarks by title or address.",
            object(json!({ "query": { "type": "string" } }), &["query"]),
            Kind::Read,
            false,
            bookmark_find,
        ),
        tool(
            "bookmark_add",
            "Bookmark a page. Without url, bookmarks the current tab.",
            object(
                json!({
                    "url": { "type": "string" },
                    "title": { "type": "string" },
                    "bar": { "type": "boolean", "description": "Put it on the bookmarks bar instead of Other bookmarks." }
                }),
                &[],
            ),
            Kind::Meta,
            false,
            bookmark_add,
        ),
        tool(
            "list_downloads",
            "List recent downloads with their file paths and states.",
            object(json!({ "limit": { "type": "integer" } }), &[]),
            Kind::Read,
            false,
            list_downloads,
        ),
        tool(
            "pin_to_board",
            "Save a picture of the current page to one of the user's boards.",
            object(
                json!({
                    "board": { "type": "string", "description": "Board name. A new board is made when none has this name." },
                    "fullPage": { "type": "boolean" }
                }),
                &[],
            ),
            Kind::Meta,
            true,
            pin_to_board,
        ),
        tool(
            "save_file",
            "Save text, such as a report, CSV or JSON, as a file in the user's Downloads\\ce-agent folder. It shows in the downloads list.",
            object(
                json!({
                    "name": { "type": "string", "description": "File name with extension, such as `report.md`." },
                    "content": { "type": "string" }
                }),
                &["name", "content"],
            ),
            Kind::Meta,
            false,
            save_file,
        ),
        tool(
            "copy_to_clipboard",
            "Put text on the clipboard.",
            object(json!({ "text": { "type": "string" } }), &["text"]),
            Kind::Meta,
            false,
            copy_to_clipboard,
        ),
        tool(
            "notify",
            "Show the user a Windows notification.",
            object(
                json!({ "title": { "type": "string" }, "body": { "type": "string" } }),
                &["body"],
            ),
            Kind::Meta,
            false,
            notify,
        ),
    ]
}

fn rows_text(rows: &[Map<String, Value>]) -> String {
    clip(
        &serde_json::to_string_pretty(rows).unwrap_or_default(),
        12_000,
    )
}

fn page_url(ctx: &Ctx) -> String {
    tabs::tab_info(ctx.app, ctx.tab)
        .map(|info| info.url)
        .unwrap_or_default()
}

fn extract_rows(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let instruction = arg_str(args, "instruction");
    if instruction.trim().is_empty() {
        return Err("Describe what to extract.".into());
    }
    let rows = extract::ai_rows(ctx.app, ctx.tab, &instruction)?;
    ctx.flow.push(FlowStep {
        kind: "extract".into(),
        prompt: instruction.clone(),
        ..flows::new_step()
    });
    let count = rows.len();
    let mut detail = format!("{count} rows:\n{}", rows_text(&rows));
    if arg_bool(args, "save", true) && count > 0 {
        let name = match arg_str(args, "name") {
            name if name.trim().is_empty() => clip(&instruction, 60),
            name => clip(name.trim(), 80),
        };
        let info = extract::save_rows(ctx.app, &name, &page_url(ctx), rows)?;
        detail.push_str(&format!("\n\nSaved as dataset \"{}\" ({}).", info.name, info.id));
    }
    step(format!("Extract {count} rows"), detail)
}

fn crawl(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let urls: Vec<String> = arg_list(args, "urls")
        .into_iter()
        .map(|url| tabs::resolve(ctx.app, &url).to_string())
        .take(50)
        .collect();
    if urls.is_empty() {
        return Err("List the addresses to visit.".into());
    }
    let instruction = arg_str(args, "instruction");
    let name = match arg_str(args, "name") {
        name if name.trim().is_empty() => clip(&instruction, 60),
        name => name,
    };
    halt(ctx)?;
    let info = extract::crawl(ctx.app, &urls, None, &instruction, &name)?;
    let rows = extract::get(ctx.app, &info.id)
        .map(|dataset| {
            clip(
                &serde_json::to_string_pretty(&dataset.data).unwrap_or_default(),
                12_000,
            )
        })
        .unwrap_or_default();
    step(
        format!("Crawl {} pages into \"{}\"", urls.len(), clip(&info.name, 40)),
        format!(
            "{} rows saved as dataset \"{}\" ({}).\n{rows}",
            info.rows, info.name, info.id
        ),
    )
}

fn list_flows(ctx: &mut Ctx, _args: &Value) -> Result<Done, String> {
    let list = flows::summaries(ctx.app);
    step(
        "List saved flows",
        if list.is_empty() {
            "The user has no saved flows.".into()
        } else {
            clip(&serde_json::to_string_pretty(&list).unwrap_or_default(), 8000)
        },
    )
}

fn run_flow(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let flow_id = arg_str(args, "flowId");
    let vars: BTreeMap<String, String> = args
        .get("vars")
        .and_then(Value::as_object)
        .map(|vars| {
            vars.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        value
                            .as_str()
                            .map(String::from)
                            .unwrap_or_else(|| value.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    halt(ctx)?;
    let run = flows::run_here(ctx.app, &flow_id, ctx.tab, "agent", vars)?;
    let finished = flows::wait_run(run, FLOW_TIMEOUT);
    tabs::set_automated(ctx.app, ctx.tab, true);
    let finished = finished.ok_or("The flow is still running after 15 minutes.")?;
    let summary = json!({
        "status": finished.status,
        "output": finished.output,
        "datasetId": finished.dataset_id,
        "steps": finished.steps.iter().map(|step| format!("{}: {}", step.status, step.label)).collect::<Vec<_>>(),
    });
    step(
        format!("Run flow \"{}\"", clip(&finished.flow_name, 60)),
        clip(
            &serde_json::to_string_pretty(&summary).unwrap_or_default(),
            8000,
        ),
    )
}

fn search_history(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let query = arg_str(args, "query");
    let limit = arg_u32(args, "limit").unwrap_or(15).clamp(1, 50) as usize;
    let hits = history_index::search(ctx.app, &query);
    let lines: Vec<String> = hits
        .into_iter()
        .take(limit)
        .map(|hit| {
            let mut line = format!("- {} <{}>", clip(&hit.title, 80), clip(&hit.url, 160));
            if !hit.snippet.trim().is_empty() {
                line.push_str(&format!(" — {}", clip(hit.snippet.trim(), 160)));
            }
            line
        })
        .collect();
    step(
        format!("Search history for \"{}\"", clip(&query, 40)),
        if lines.is_empty() {
            "Nothing in the history matches.".into()
        } else {
            lines.join("\n")
        },
    )
}

fn collect_links(node: &BookmarkNode, path: &str, out: &mut Vec<(String, String, String)>) {
    if let Some(url) = &node.url {
        out.push((node.title.clone(), url.clone(), path.to_string()));
    }
    for child in node.children.iter().flatten() {
        let inner = if child.url.is_none() && !child.separator {
            format!("{path}/{}", child.title)
        } else {
            path.to_string()
        };
        collect_links(child, &inner, out);
    }
}

fn bookmark_find(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let query = arg_str(args, "query").to_lowercase();
    let tree = bookmarks::tree(ctx.app);
    let mut links = Vec::new();
    collect_links(&tree.bar, "Bookmarks bar", &mut links);
    collect_links(&tree.other, "Other bookmarks", &mut links);
    let lines: Vec<String> = links
        .into_iter()
        .filter(|(title, url, _)| {
            query.trim().is_empty()
                || title.to_lowercase().contains(query.trim())
                || url.to_lowercase().contains(query.trim())
        })
        .take(40)
        .map(|(title, url, path)| format!("- {} <{}> in {path}", clip(&title, 80), clip(&url, 160)))
        .collect();
    step(
        format!("Search bookmarks for \"{}\"", clip(&query, 40)),
        if lines.is_empty() {
            "No bookmarks match.".into()
        } else {
            lines.join("\n")
        },
    )
}

fn bookmark_add(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let info = tabs::tab_info(ctx.app, ctx.tab);
    let url = match arg_str(args, "url") {
        url if url.trim().is_empty() => info.as_ref().map(|info| info.url.clone()).unwrap_or_default(),
        url => tabs::resolve(ctx.app, &url).to_string(),
    };
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only web pages can be bookmarked.".into());
    }
    let title = match arg_str(args, "title") {
        title if title.trim().is_empty() => info
            .filter(|info| info.url == url)
            .map(|info| info.title)
            .unwrap_or_else(|| url.clone()),
        title => clip(title.trim(), 200),
    };
    if bookmarks::find_by_url(ctx.app, &url).is_some() {
        return step(
            format!("Bookmark \"{}\"", clip(&title, 60)),
            "It was already bookmarked.",
        );
    }
    let parent = if arg_bool(args, "bar", false) {
        bookmarks::BAR
    } else {
        bookmarks::OTHER
    };
    bookmarks::add(ctx.app, parent, &title, Some(url.clone()), None, false)?;
    step(format!("Bookmark \"{}\"", clip(&title, 60)), "Bookmarked.")
}

fn list_downloads(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let limit = arg_u32(args, "limit").unwrap_or(15).clamp(1, 100) as usize;
    let mut items = downloads::list(ctx.app);
    items.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    let lines: Vec<String> = items
        .into_iter()
        .take(limit)
        .map(|item| {
            format!(
                "- {} ({}) {} from <{}>",
                item.file_name,
                item.state,
                item.path,
                clip(&item.url, 120)
            )
        })
        .collect();
    step(
        "List downloads",
        if lines.is_empty() {
            "There are no downloads.".into()
        } else {
            lines.join("\n")
        },
    )
}

fn pin_to_board(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let wanted = arg_str(args, "board");
    let snapshot = boards::snapshot(ctx.app);
    let board = if wanted.trim().is_empty() {
        snapshot
            .last_board_id
            .as_ref()
            .and_then(|id| snapshot.boards.iter().find(|board| &board.id == id))
            .or_else(|| snapshot.boards.first())
            .cloned()
    } else {
        snapshot
            .boards
            .iter()
            .find(|board| board.name.eq_ignore_ascii_case(wanted.trim()))
            .cloned()
    };
    let board = match board {
        Some(board) => board,
        None => boards::create_board(
            ctx.app,
            if wanted.trim().is_empty() {
                "Agent"
            } else {
                wanted.trim()
            },
            None,
        )?,
    };
    halt(ctx)?;
    boards::pin_page(ctx.app, ctx.tab, &board.id, arg_bool(args, "fullPage", false))?;
    step(
        format!("Pin the page to \"{}\"", clip(&board.name, 40)),
        format!("Pinned to the board \"{}\".", board.name),
    )
}

fn mime_of(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "xml" => "application/xml",
        _ => "text/plain",
    }
}

fn save_file(ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let name = files::sanitize_file_name(arg_str(args, "name").trim());
    if name.is_empty() {
        return Err("Name the file.".into());
    }
    let name = if name.contains('.') {
        name
    } else {
        format!("{name}.txt")
    };
    let content = arg_str(args, "content");
    if content.len() > 5_000_000 {
        return Err("That file is too large.".into());
    }
    let dir = settings::get(ctx.app).download_dir(ctx.app).join("ce-agent");
    fs::create_dir_all(&dir).map_err(|error| format!("Couldn't make the folder. {error}"))?;
    let path = files::unique_path(&dir, &name);
    files::write_text_file(&path, &content)?;
    let url = url::Url::from_file_path(&path)
        .map(|url| url.to_string())
        .unwrap_or_default();
    downloads::record_completed(ctx.app, &url, &path, mime_of(&name));
    step(
        format!(
            "Save {}",
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or(name)
        ),
        format!("Saved to {}.", path.display()),
    )
}

fn copy_to_clipboard(_ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let text = arg_str(args, "text");
    ocr::copy_text(&text)?;
    step(
        format!("Copy \"{}\"", clip(&text, 60)),
        "Copied to the clipboard.",
    )
}

fn notify(_ctx: &mut Ctx, args: &Value) -> Result<Done, String> {
    let title = match arg_str(args, "title") {
        title if title.trim().is_empty() => "Agent".to_string(),
        title => clip(title.trim(), 80),
    };
    let body = clip(arg_str(args, "body").trim(), 300);
    system::toast(&title, &body, "");
    step(format!("Notify: {}", clip(&body, 60)), "Sent.")
}
