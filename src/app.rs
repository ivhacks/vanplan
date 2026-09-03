use crate::hash;
use crate::layout::{self, NODE_H, NODE_W};
use crate::model::{self, Document, Edge, Node};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, BlobPropertyBag, CustomEvent, CustomEventInit, HashChangeEvent, HtmlElement,
    HtmlInputElement, KeyboardEvent, PointerEvent, Response, Url,
};
use yew::prelude::*;

#[derive(Clone, Copy)]
enum WireDir {
    /// Started on the left circle: this card is the dependent.
    FromDep,
    /// Started on the right circle: this card is the prereq.
    FromPre,
}

#[derive(Clone)]
struct Wiring {
    source: String,
    dir: WireDir,
    x: f64,
    y: f64,
}

#[derive(Clone)]
struct State {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    pos: HashMap<String, (f64, f64)>,
    next_n: u32,
    selected: Option<String>,
    wiring: Option<Wiring>,
    focus_id: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            pos: HashMap::new(),
            next_n: 1,
            selected: None,
            wiring: None,
            focus_id: None,
        }
    }
}

impl State {
    fn relayout(&mut self) {
        let ids: Vec<String> = self.nodes.iter().map(|n| n.id.clone()).collect();
        let edges: Vec<(String, String)> = self
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();
        self.pos = layout::layout(&ids, &edges);
    }

    fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
}

enum Msg {
    AddNode,
    SetText {
        id: String,
        text: String,
    },
    CycleColor {
        id: String,
    },
    ToggleDone {
        id: String,
    },
    StartWire {
        source: String,
        dir: WireDir,
        x: f64,
        y: f64,
    },
    MoveWire {
        x: f64,
        y: f64,
    },
    EndWire {
        target: Option<String>,
    },
    DeleteWire {
        from: String,
        to: String,
    },
    DeleteNode {
        id: String,
    },
    Select {
        id: Option<String>,
    },
    Load(Document),
}

impl Reducible for State {
    type Action = Msg;

    fn reduce(self: Rc<Self>, action: Msg) -> Rc<Self> {
        let mut s = (*self).clone();
        match action {
            Msg::AddNode => {
                let id = model::fresh_id(&mut s.next_n, &s.nodes);
                s.nodes.push(Node {
                    id: id.clone(),
                    text: String::new(),
                    color: model::COLORS[0].to_string(),
                    done: false,
                });
                s.selected = Some(id.clone());
                s.focus_id = Some(id);
                s.relayout();
            }
            Msg::SetText { id, text } => {
                if let Some(i) = s.node_index(&id) {
                    s.nodes[i].text = text;
                }
            }
            Msg::CycleColor { id } => {
                if let Some(i) = s.node_index(&id) {
                    s.nodes[i].color = model::next_color(&s.nodes[i].color).to_string();
                }
            }
            Msg::ToggleDone { id } => {
                if let Some(i) = s.node_index(&id) {
                    s.nodes[i].done = !s.nodes[i].done;
                }
            }
            Msg::StartWire { source, dir, x, y } => {
                s.wiring = Some(Wiring { source, dir, x, y });
            }
            Msg::MoveWire { x, y } => {
                if let Some(w) = s.wiring.as_mut() {
                    w.x = x;
                    w.y = y;
                }
            }
            Msg::EndWire { target } => {
                if let Some(w) = s.wiring.take() {
                    if let Some(other) = target {
                        let (from, to) = match w.dir {
                            WireDir::FromDep => (other, w.source),
                            WireDir::FromPre => (w.source, other),
                        };
                        model::add_edge(&mut s.edges, from, to);
                        s.relayout();
                    }
                }
            }
            Msg::DeleteWire { from, to } => {
                s.edges.retain(|e| !(e.from == from && e.to == to));
                s.relayout();
            }
            Msg::DeleteNode { id } => {
                s.nodes.retain(|n| n.id != id);
                s.edges.retain(|e| e.from != id && e.to != id);
                if s.selected.as_deref() == Some(id.as_str()) {
                    s.selected = None;
                }
                if s.focus_id.as_deref() == Some(id.as_str()) {
                    s.focus_id = None;
                }
                s.relayout();
            }
            Msg::Select { id } => {
                s.selected = id;
            }
            Msg::Load(doc) => {
                s.nodes = doc.nodes;
                s.edges = doc.edges;
                s.selected = None;
                s.wiring = None;
                s.focus_id = None;
                s.relayout();
            }
        }
        Rc::new(s)
    }
}

fn window() -> web_sys::Window {
    web_sys::window().expect("window")
}

fn location_hash() -> String {
    window().location().hash().unwrap_or_default()
}

fn console_error(msg: &str) {
    web_sys::console::error_1(&JsValue::from_str(msg));
}

fn load_from_hash_str(hash: &str) -> Result<Option<Document>, String> {
    let h = hash.trim();
    let rest = h.strip_prefix('#').unwrap_or(h).trim();
    if rest.is_empty() {
        return Ok(None);
    }
    let yaml = hash::decode(h)?;
    model::from_yaml(&yaml).map(Some)
}

fn initial_state() -> State {
    let mut s = State::default();
    match load_from_hash_str(&location_hash()) {
        Ok(Some(doc)) => {
            s.nodes = doc.nodes;
            s.edges = doc.edges;
            s.relayout();
        }
        Ok(None) => {}
        Err(err) => console_error(&format!("vanplan hash: {err}")),
    }
    s
}

fn replace_doc_hash(payload: &str) {
    let cur = location_hash();
    let cur = cur.strip_prefix('#').unwrap_or(&cur);
    if cur == payload {
        return;
    }
    let Ok(history) = window().history() else {
        return;
    };
    let url = format!("#{payload}");
    let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url));
}

fn install_vanplan_hook() {
    let api = js_sys::Object::new();

    let yaml_cb = Closure::<dyn FnMut() -> String>::wrap(Box::new(|| {
        let hash = location_hash();
        match hash::decode(&hash) {
            Ok(yaml) => yaml,
            Err(_) => model::to_yaml(&[], &[]).unwrap_or_default(),
        }
    }));
    let _ = js_sys::Reflect::set(&api, &JsValue::from_str("yaml"), yaml_cb.as_ref());
    yaml_cb.forget();

    let load_cb = Closure::<dyn FnMut(String)>::wrap(Box::new(|yaml: String| {
        let init = CustomEventInit::new();
        init.set_detail(&JsValue::from_str(&yaml));
        if let Ok(ev) = CustomEvent::new_with_event_init_dict("vanplan-load", &init) {
            let _ = window().dispatch_event(&ev);
        }
    }));
    let _ = js_sys::Reflect::set(&api, &JsValue::from_str("load"), load_cb.as_ref());
    load_cb.forget();

    let href_cb = Closure::<dyn FnMut() -> String>::wrap(Box::new(|| {
        window().location().href().unwrap_or_default()
    }));
    let _ = js_sys::Reflect::set(&api, &JsValue::from_str("href"), href_cb.as_ref());
    href_cb.forget();

    let _ = js_sys::Reflect::set(&window(), &JsValue::from_str("vanplan"), &api);
}

fn document() -> web_sys::Document {
    window().document().expect("document")
}

fn canvas_xy(canvas: &HtmlElement, client_x: f64, client_y: f64) -> (f64, f64) {
    let r = canvas.get_bounding_client_rect();
    (
        client_x - r.left() + f64::from(canvas.scroll_left()),
        client_y - r.top() + f64::from(canvas.scroll_top()),
    )
}

fn hit_other_node(client_x: i32, client_y: i32, exclude_id: &str) -> Option<String> {
    let el = document().element_from_point(client_x as f32, client_y as f32)?;
    let id = el
        .closest("[data-port]")
        .ok()
        .flatten()
        .and_then(|p| p.get_attribute("data-id"))
        .or_else(|| {
            el.closest(".node")
                .ok()
                .flatten()
                .and_then(|n| n.get_attribute("data-id"))
        })?;
    if id == exclude_id {
        None
    } else {
        Some(id)
    }
}

fn download_yaml(yaml: &str) {
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from_str(yaml));
    let opts = BlobPropertyBag::new();
    opts.set_type("application/x-yaml");
    let Ok(blob) = Blob::new_with_str_sequence_and_options(&arr, &opts) else {
        return;
    };
    let Ok(url) = Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(a) = document().create_element("a") {
        if let Ok(a) = a.dyn_into::<web_sys::HtmlAnchorElement>() {
            a.set_href(&url);
            a.set_download("plan.vanplan");
            a.click();
        }
    }
    let _ = Url::revoke_object_url(&url);
}

#[function_component(App)]
pub fn app() -> Html {
    let state = use_reducer(initial_state);
    let canvas_ref = use_node_ref();
    let file_ref = use_node_ref();

    let snap = use_mut_ref(|| {
        (
            Vec::<Node>::new(),
            Vec::<Edge>::new(),
            Option::<String>::None,
        )
    });
    {
        let mut g = snap.borrow_mut();
        g.0 = state.nodes.clone();
        g.1 = state.edges.clone();
        g.2 = state.selected.clone();
    }

    {
        let nodes = state.nodes.clone();
        let edges = state.edges.clone();
        use_effect_with((nodes, edges), move |(nodes, edges)| {
            let skip = nodes.is_empty()
                && edges.is_empty()
                && location_hash()
                    .trim()
                    .trim_start_matches('#')
                    .is_empty();
            if !skip {
                match model::to_yaml(nodes, edges).and_then(|y| hash::encode(&y)) {
                    Ok(payload) => replace_doc_hash(&payload),
                    Err(err) => console_error(&err),
                }
            }
            || {}
        });
    }

    {
        let dispatcher = state.dispatcher();
        use_effect_with((), move |_| {
            install_vanplan_hook();
            let win = window();
            let win2 = win.clone();
            let win3 = win.clone();
            let d_hash = dispatcher.clone();
            let hash_cb = Closure::<dyn FnMut(HashChangeEvent)>::wrap(Box::new(
                move |_e: HashChangeEvent| match load_from_hash_str(&location_hash()) {
                    Ok(Some(doc)) => d_hash.dispatch(Msg::Load(doc)),
                    Ok(None) => d_hash.dispatch(Msg::Load(Document {
                        vanplan: 1,
                        nodes: Vec::new(),
                        edges: Vec::new(),
                    })),
                    Err(err) => console_error(&format!("vanplan hash: {err}")),
                },
            ));
            let d_load = dispatcher.clone();
            let load_cb =
                Closure::<dyn FnMut(CustomEvent)>::wrap(Box::new(move |e: CustomEvent| {
                    let Some(yaml) = e.detail().as_string() else {
                        return;
                    };
                    match model::from_yaml(&yaml) {
                        Ok(doc) => d_load.dispatch(Msg::Load(doc)),
                        Err(err) => console_error(&err),
                    }
                }));
            let _ = win
                .add_event_listener_with_callback("hashchange", hash_cb.as_ref().unchecked_ref());
            let _ = win
                .add_event_listener_with_callback("vanplan-load", load_cb.as_ref().unchecked_ref());
            move || {
                let _ = win2.remove_event_listener_with_callback(
                    "hashchange",
                    hash_cb.as_ref().unchecked_ref(),
                );
                let _ = win3.remove_event_listener_with_callback(
                    "vanplan-load",
                    load_cb.as_ref().unchecked_ref(),
                );
            }
        });
    }

    {
        let dispatcher = state.dispatcher();
        use_effect_with((), move |_| {
            if location_hash()
                .trim()
                .trim_start_matches('#')
                .is_empty()
            {
            wasm_bindgen_futures::spawn_local(async move {
                let Ok(resp_val) = JsFuture::from(window().fetch_with_str("seed.vanplan")).await
                else {
                    return;
                };
                let Ok(resp) = resp_val.dyn_into::<Response>() else {
                    return;
                };
                if !resp.ok() {
                    return;
                }
                let Ok(text_p) = resp.text() else {
                    return;
                };
                let Ok(text_val) = JsFuture::from(text_p).await else {
                    return;
                };
                let Some(yaml) = text_val.as_string() else {
                    return;
                };
                match model::from_yaml(&yaml) {
                    Ok(doc) => dispatcher.dispatch(Msg::Load(doc)),
                    Err(err) => console_error(&err),
                }
            });
            }
            || {}
        });
    }

    {
        let snap = snap.clone();
        let dispatcher = state.dispatcher();
        let file_ref = file_ref.clone();
        use_effect_with((), move |_| {
            let doc = document();
            let doc2 = doc.clone();
            let closure =
                Closure::<dyn FnMut(KeyboardEvent)>::wrap(Box::new(move |e: KeyboardEvent| {
                    let key = e.key();
                    let cmd = e.meta_key() || e.ctrl_key();
                    if cmd && key.eq_ignore_ascii_case("s") {
                        e.prevent_default();
                        let g = snap.borrow();
                        if let Ok(yaml) = model::to_yaml(&g.0, &g.1) {
                            download_yaml(&yaml);
                        }
                        return;
                    }
                    if cmd && key.eq_ignore_ascii_case("o") {
                        e.prevent_default();
                        if let Some(input) = file_ref.cast::<HtmlInputElement>() {
                            input.click();
                        }
                        return;
                    }
                    if key == "Backspace" || key == "Delete" {
                        let t = e
                            .target()
                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                        let typing = t
                            .as_ref()
                            .map(|el| {
                                let tag = el.tag_name();
                                tag == "INPUT"
                                    || tag == "TEXTAREA"
                                    || el.get_attribute("contenteditable").is_some()
                            })
                            .unwrap_or(false);
                        if typing {
                            return;
                        }
                        let selected = snap.borrow().2.clone();
                        if let Some(id) = selected {
                            e.prevent_default();
                            dispatcher.dispatch(Msg::DeleteNode { id });
                        }
                    }
                }));
            let _ =
                doc.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
            move || {
                let _ = doc2.remove_event_listener_with_callback(
                    "keydown",
                    closure.as_ref().unchecked_ref(),
                );
            }
        });
    }

    {
        let state = state.clone();
        let canvas_ref = canvas_ref.clone();
        let active = state.wiring.is_some();
        use_effect_with(active, move |active| {
            if !*active {
                return Box::new(|| {}) as Box<dyn FnOnce()>;
            }
            let win = window();
            let win2 = win.clone();
            let win3 = win.clone();
            let state_m = state.clone();
            let canvas_m = canvas_ref.clone();
            let move_cb =
                Closure::<dyn FnMut(PointerEvent)>::wrap(Box::new(move |e: PointerEvent| {
                    if let Some(canvas) = canvas_m.cast::<HtmlElement>() {
                        let (x, y) = canvas_xy(&canvas, e.client_x() as f64, e.client_y() as f64);
                        state_m.dispatch(Msg::MoveWire { x, y });
                    }
                }));
            let state_u = state.clone();
            let up_cb =
                Closure::<dyn FnMut(PointerEvent)>::wrap(Box::new(move |e: PointerEvent| {
                    let target = state_u
                        .wiring
                        .as_ref()
                        .and_then(|w| hit_other_node(e.client_x(), e.client_y(), &w.source));
                    state_u.dispatch(Msg::EndWire { target });
                }));
            let _ = win
                .add_event_listener_with_callback("pointermove", move_cb.as_ref().unchecked_ref());
            let _ =
                win.add_event_listener_with_callback("pointerup", up_cb.as_ref().unchecked_ref());
            Box::new(move || {
                let _ = win2.remove_event_listener_with_callback(
                    "pointermove",
                    move_cb.as_ref().unchecked_ref(),
                );
                let _ = win3.remove_event_listener_with_callback(
                    "pointerup",
                    up_cb.as_ref().unchecked_ref(),
                );
            }) as Box<dyn FnOnce()>
        });
    }

    let on_dblclick = {
        let state = state.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(t) = e.target_dyn_into::<web_sys::Element>() {
                if t.closest(".node").ok().flatten().is_some() {
                    return;
                }
                if t.closest(".wire-hit").ok().flatten().is_some() {
                    return;
                }
            }
            state.dispatch(Msg::AddNode);
        })
    };

    let on_canvas_click = {
        let state = state.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(t) = e.target_dyn_into::<web_sys::Element>() {
                if t.closest(".node").ok().flatten().is_some() {
                    return;
                }
            }
            state.dispatch(Msg::Select { id: None });
        })
    };

    let on_file = {
        let state = state.clone();
        Callback::from(move |e: Event| {
            let Some(input) = e.target_dyn_into::<HtmlInputElement>() else {
                return;
            };
            let Some(files) = input.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };
            input.set_value("");
            let promise = file.text();
            let state = state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match JsFuture::from(promise).await {
                    Ok(js) => {
                        if let Some(text) = js.as_string() {
                            match model::from_yaml(&text) {
                                Ok(doc) => state.dispatch(Msg::Load(doc)),
                                Err(err) => {
                                    web_sys::console::error_1(&JsValue::from_str(&err));
                                }
                            }
                        }
                    }
                    Err(err) => web_sys::console::error_1(&err),
                }
            });
        })
    };

    let max_x = state
        .pos
        .values()
        .map(|p| p.0 + NODE_W + layout::PAD)
        .fold(0.0, f64::max)
        .max(800.0);
    let max_y = state
        .pos
        .values()
        .map(|p| p.1 + NODE_H + layout::PAD)
        .fold(0.0, f64::max)
        .max(600.0);

    let wires: Html = state
        .edges
        .iter()
        .filter_map(|e| {
            let (dx, dy) = *state.pos.get(&e.to)?;
            let (px, py) = *state.pos.get(&e.from)?;
            let (x1, y1) = layout::port_left(dx, dy);
            let (x2, y2) = layout::port_right(px, py);
            let d = layout::wire_d(x1, y1, x2, y2);
            let from = e.from.clone();
            let to = e.to.clone();
            let state = state.clone();
            let onclick = Callback::from(move |ev: MouseEvent| {
                ev.stop_propagation();
                state.dispatch(Msg::DeleteWire {
                    from: from.clone(),
                    to: to.clone(),
                });
            });
            let key = format!("{}->{}", e.from, e.to);
            Some(html! {
                <g key={key}>
                    <path class="wire-hit" d={d.clone()} onclick={onclick} />
                    <path class="wire" d={d} />
                </g>
            })
        })
        .collect();

    let temp_wire = state.wiring.as_ref().and_then(|w| {
        let (nx, ny) = *state.pos.get(&w.source)?;
        let (x1, y1) = match w.dir {
            WireDir::FromDep => layout::port_left(nx, ny),
            WireDir::FromPre => layout::port_right(nx, ny),
        };
        let d = layout::wire_d(x1, y1, w.x, w.y);
        Some(html! { <path class="wire temp" d={d} /> })
    });

    let cards: Html = state
        .nodes
        .iter()
        .map(|n| {
            let (x, y) = state
                .pos
                .get(&n.id)
                .copied()
                .unwrap_or((layout::PAD, layout::PAD));
            let selected = state.selected.as_deref() == Some(n.id.as_str());
            let autofocus = state.focus_id.as_deref() == Some(n.id.as_str());
            html! {
                <NodeCard
                    key={n.id.clone()}
                    node={n.clone()}
                    x={x}
                    y={y}
                    selected={selected}
                    autofocus={autofocus}
                    dispatcher={state.dispatcher()}
                    canvas_ref={canvas_ref.clone()}
                />
            }
        })
        .collect();

    html! {
        <div
            class="canvas"
            ref={canvas_ref}
            ondblclick={on_dblclick}
            onclick={on_canvas_click}
        >
            <svg class="wires" width={max_x.to_string()} height={max_y.to_string()}>
                { wires }
                { temp_wire }
            </svg>
            { cards }
            <input
                type="file"
                accept=".vanplan,.yaml,.yml"
                ref={file_ref}
                class="file"
                onchange={on_file}
            />
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
struct NodeCardProps {
    node: Node,
    x: f64,
    y: f64,
    selected: bool,
    autofocus: bool,
    dispatcher: UseReducerDispatcher<State>,
    canvas_ref: NodeRef,
}

#[function_component(NodeCard)]
fn node_card(props: &NodeCardProps) -> Html {
    let input_ref = use_node_ref();
    {
        let input_ref = input_ref.clone();
        let autofocus = props.autofocus;
        use_effect_with(autofocus, move |af| {
            if *af {
                if let Some(el) = input_ref.cast::<HtmlInputElement>() {
                    let _ = el.focus();
                }
            }
            || {}
        });
    }

    let id = props.node.id.clone();
    let on_input = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            dispatcher.dispatch(Msg::SetText {
                id: id.clone(),
                text: input.value(),
            });
        })
    };
    let on_cycle = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dispatcher.dispatch(Msg::CycleColor { id: id.clone() });
        })
    };
    let on_toggle_done = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dispatcher.dispatch(Msg::ToggleDone { id: id.clone() });
        })
    };
    let on_delete = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dispatcher.dispatch(Msg::DeleteNode { id: id.clone() });
        })
    };
    let on_select = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dispatcher.dispatch(Msg::Select {
                id: Some(id.clone()),
            });
        })
    };
    let on_start = {
        let dispatcher = props.dispatcher.clone();
        let id = id.clone();
        let canvas_ref = props.canvas_ref.clone();
        Callback::from(move |e: PointerEvent| {
            e.stop_propagation();
            e.prevent_default();
            let el: HtmlElement = e.target_unchecked_into();
            let _ = el.set_pointer_capture(e.pointer_id());
            let (x, y) = if let Some(canvas) = canvas_ref.cast::<HtmlElement>() {
                canvas_xy(&canvas, e.client_x() as f64, e.client_y() as f64)
            } else {
                (e.client_x() as f64, e.client_y() as f64)
            };
            let dir = match el.get_attribute("data-port").as_deref() {
                Some("pre") => WireDir::FromPre,
                _ => WireDir::FromDep,
            };
            dispatcher.dispatch(Msg::StartWire {
                source: id.clone(),
                dir,
                x,
                y,
            });
        })
    };
    let on_dbl = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let fg = model::text_color(&props.node.color);
    let style = format!(
        "left:{:.0}px;top:{:.0}px;background:{};color:{};",
        props.x, props.y, props.node.color, fg
    );
    let class = match (props.selected, props.node.done) {
        (true, true) => "node selected done",
        (true, false) => "node selected",
        (false, true) => "node done",
        (false, false) => "node",
    };

    html! {
        <div class={class} style={style} data-id={props.node.id.clone()} onclick={on_select} ondblclick={on_dbl}>
            <div
                class="port dep"
                data-port="dep"
                data-id={props.node.id.clone()}
                onpointerdown={on_start.clone()}
            />
            <button type="button" class="check" tabindex="-1" onclick={on_toggle_done}>
                { if props.node.done { "✓" } else { "" } }
            </button>
            <input
                ref={input_ref}
                type="text"
                value={props.node.text.clone()}
                oninput={on_input}
            />
            <div
                class="swatch"
                style={format!("background:{};", props.node.color)}
                onclick={on_cycle}
            />
            <button type="button" class="x" tabindex="-1" onclick={on_delete}>{ "×" }</button>
            <div
                class="port pre"
                data-port="pre"
                data-id={props.node.id.clone()}
                onpointerdown={on_start}
            />
        </div>
    }
}
