//! The WebSocket to the game (`/studio`): requests and what the game pushes.

use std::cell::{Cell, RefCell};

use leptos::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::state::*;

/// Sends a request to the game; `then` gets its answer.
pub(crate) fn request(method: &str, params: Value, then: impl FnOnce(Result<Value, String>) + 'static) {
    let id = NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    let sent = SOCKET.with(|s| {
        s.borrow()
            .as_ref()
            .and_then(|ws| ws.send_with_str(&json!({ "id": id, "method": method, "params": params }).to_string()).ok())
            .is_some()
    });
    if sent {
        PENDING.with(|p| p.borrow_mut().insert(id, Box::new(then)));
    } else {
        then(Err("not connected to the game".into()));
    }
}

pub(crate) fn connect(s: S) {
    let token = TOKEN.with(|t| t.borrow().clone());
    if token.is_empty() {
        s.pairing.set(Pairing::Needed);
        return;
    }
    let url = format!("ws://{}/studio?token={token}", host());
    let Ok(ws) = web_sys::WebSocket::new(&url) else { return };
    let opened = std::rc::Rc::new(Cell::new(false));
    {
        let opened = opened.clone();
        let on_open = Closure::<dyn FnMut()>::new(move || {
            opened.set(true);
            s.connected.set(true);
            s.pairing.set(Pairing::Paired);
            if let Some(st) = storage() {
                let _ = st.set_item("glue5-token", &TOKEN.with(|t| t.borrow().clone()));
            }
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();
    }
    let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
        let Some(text) = e.data().as_string() else { return };
        let Ok(msg) = serde_json::from_str::<Value>(&text) else { return };
        if let Some(id) = msg["id"].as_u64() {
            if let Some(then) = PENDING.with(|p| p.borrow_mut().remove(&id)) {
                then(match msg.get("error") {
                    Some(e) if !e.is_null() => Err(e.as_str().unwrap_or("error").to_string()),
                    _ => Ok(msg["result"].clone()),
                });
            }
            return;
        }
        match msg["method"].as_str() {
            Some("hello") => s.hello.set(msg["params"].clone()),
            Some("mods") => {
                if let Ok(mods) = serde_json::from_value::<Vec<ModInfo>>(msg["params"].clone())
                    && s.mods.with_untracked(|m| *m != mods)
                {
                    s.mods.set(mods);
                }
            }
            Some("menus") => {
                if let Ok(menus) = serde_json::from_value::<Vec<crate::menus::MenuInfo>>(msg["params"].clone()) {
                    s.menus.set(menus);
                }
            }
            Some("log") => {
                if let Ok(lines) = serde_json::from_value::<Vec<LogLine>>(msg["params"].clone()) {
                    for l in lines {
                        push_row(s, Row::Log(l));
                    }
                }
            }
            Some("files") => {
                for path in msg["params"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    reload(s, path);
                }
            }
            _ => {}
        }
    });
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    let on_close = Closure::<dyn FnMut()>::new(move || {
        s.connected.set(false);
        SOCKET.with(|sock| sock.borrow_mut().take());
        PENDING.with(|p| {
            for (_, then) in p.borrow_mut().drain() {
                then(Err("the connection to the game closed".into()));
            }
        });
        if opened.get() {
            // The game went away (or the network did): try again.
            set_timeout(move || connect(s), std::time::Duration::from_secs(2));
        } else {
            // A new code is needed: the editor stops trying the old one too.
            with_editor(|e| e.pause());
            s.pairing.set(Pairing::Needed);
        }
    });
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();
    SOCKET.with(|sock| *sock.borrow_mut() = Some(ws));
}

/// A file changed in the game's folder: an open, unchanged tab follows it.
pub(crate) fn reload(s: S, path: &str) {
    let tab = s.tabs.with_untracked(|t| t.iter().find(|t| t.path == path).cloned());
    let Some(tab) = tab else { return };
    if tab.read_only {
        return;
    }
    if tab.dirty {
        // Saving would overwrite what changed: say so.
        s.notice.set(Some(format!("{path} changed in the game's folder; saving keeps your version")));
        return;
    }
    let p = path.to_string();
    request("files.read", json!({ "path": path }), move |r| {
        if let Ok(v) = r {
            with_editor(|e| e.replace(&p, v["text"].as_str().unwrap_or("")));
        }
    });
}

/// Fetches `url` as text (no futures runtime: the promise's callbacks).
pub(crate) fn fetch_text(url: String, then: impl FnOnce(String) + 'static) {
    let Some(window) = web_sys::window() else { return };
    let then = RefCell::new(Some(then));
    let on_text = Closure::<dyn FnMut(JsValue)>::new(move |text: JsValue| {
        if let Some(f) = then.borrow_mut().take() {
            f(text.as_string().unwrap_or_default());
        }
    });
    let on_response = Closure::<dyn FnMut(JsValue)>::new(move |resp: JsValue| {
        if let Ok(text) = js_sys::Reflect::get(&resp, &"text".into()).and_then(|f| f.dyn_into::<js_sys::Function>())
            && let Ok(p) = text.call0(&resp).and_then(|p| p.dyn_into::<js_sys::Promise>())
        {
            let _ = p.then(&on_text);
        }
    });
    let _ = window.fetch_with_str(&url).then(&on_response);
    on_response.forget();
}

/// Uploads a selected ZIP to the paired game's authenticated mod importer.
pub(crate) fn import_zip(s: S, file: web_sys::File, name: String) {
    let Some(window) = web_sys::window() else { s.notice.set(Some("Browser window unavailable".into())); return };
    let Ok(reader) = web_sys::FileReader::new() else { s.notice.set(Some("Could not read ZIP file".into())); return };
    let load_reader = reader.clone();
    let on_load = Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(move |_| {
        let Ok(value) = load_reader.result() else { s.notice.set(Some("Could not read ZIP data".into())); return };
        let bytes = js_sys::Uint8Array::new(&value);
        let mut init = web_sys::RequestInit::new();
        init.set_method("POST");
        let body: wasm_bindgen::JsValue = bytes.into();
        init.set_body(&body);
        let headers = match web_sys::Headers::new() {
            Ok(h) => h,
            Err(_) => { s.notice.set(Some("Could not prepare ZIP upload".into())); return; }
        };
        if headers.set("Content-Type", "application/zip").is_err() {
            s.notice.set(Some("Could not prepare ZIP upload".into()));
            return;
        }
        init.set_headers(&headers.into());
        let token = TOKEN.with(|t| t.borrow().clone());
        let url = format!(
            "http://{}/mods/import?name={}&token={}",
            host(),
            js_sys::encode_uri_component(&name),
            js_sys::encode_uri_component(&token)
        );
        let promise = window.fetch_with_str_and_init(&url, &init);
        let response_name = name.clone();
        let on_response = Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(move |value: wasm_bindgen::JsValue| {
            let response: web_sys::Response = value.unchecked_into();
            let ok = response.ok();
            let text: js_sys::Promise = match response.text() {
                Ok(p) => p,
                Err(_) => { s.notice.set(Some("The game returned an unreadable response".into())); return; }
            };
            let imported_name = response_name.clone();
            let on_text = Closure::<dyn FnMut(wasm_bindgen::JsValue)>::new(move |value: wasm_bindgen::JsValue| {
                let message = value.as_string().unwrap_or_default();
                if ok {
                    s.notice.set(Some(format!("Mod '{}' imported. Enable it in Mods.", imported_name)));
                } else {
                    let error = serde_json::from_str::<serde_json::Value>(&message)
                        .ok().and_then(|v| v["error"].as_str().map(str::to_string))
                        .unwrap_or_else(|| format!("ZIP import failed: {}", message));
                    s.notice.set(Some(error));
                }
            });
            let _ = text.then(&on_text);
            on_text.forget();
        });
        let _ = promise.then(&on_response);
        on_response.forget();
    });
    reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
    on_load.forget();
    if reader.read_as_array_buffer(&file).is_err() {
        s.notice.set(Some("Could not open selected ZIP file".into()));
    } else {
        s.notice.set(Some(format!("Reading {}…", file.name())));
    }
}
