use crate::remote_control::common::*;
use crate::remote_control::property::*;

use std::fmt::Write as _;

/// Escape text before embedding into HTML.
fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Build the static control list HTML, including an auto-injected quit button.
pub(crate) fn render_controls(properties: &[PropertyDefinition]) -> String {
    let mut out = String::new();
    for property in properties {
        let prop_id = property.lookup_id();
        let label = escape_html(&property.label);

        match property.control {
            PropertyControl::Slider {
                min,
                max,
                step,
                initial,
            } => {
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}: <span class=\"value\" id=\"value-{prop_id}\">{initial}</span></label>\
                        <input type=\"range\" min=\"{min}\" max=\"{max}\" step=\"{step}\" value=\"{initial}\" \
                               oninput=\"setLabel('{prop_id}', this.value)\" \
                               onchange=\"sendUpdate('{prop_id}', this.value)\">\
                    </div>"
                );
            }
            PropertyControl::Toggle { initial } => {
                let checked = if initial { " checked" } else { "" };
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>\
                            <input type=\"checkbox\"{checked} onchange=\"sendUpdate('{prop_id}', this.checked ? '1' : '0')\">\
                            {label}\
                        </label>\
                    </div>"
                );
            }
            PropertyControl::Select {
                ref options,
                initial,
            } => {
                let safe_initial = if options.is_empty() {
                    0
                } else {
                    initial.min(options.len() - 1)
                };
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}</label>\
                        <select onchange=\"sendUpdate('{prop_id}', this.value)\">"
                );
                for (idx, option) in options.iter().enumerate() {
                    let selected = if idx == safe_initial { " selected" } else { "" };
                    let escaped = escape_html(option);
                    let _ = write!(
                        out,
                        "<option value=\"{escaped}\"{selected}>{escaped}</option>"
                    );
                }
                out.push_str("</select></div>");
            }
            PropertyControl::String { ref initial } => {
                let escaped_initial = escape_html(initial);
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}</label>\
                        <input type=\"text\" value=\"{escaped_initial}\" onchange=\"sendUpdate('{prop_id}', this.value)\">\
                    </div>"
                );
            }
            PropertyControl::Vector3 { initial, step } => {
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}</label>\
                        <div class=\"vec3\">\
                            <input id=\"vec3-{prop_id}-x\" type=\"number\" step=\"{step}\" value=\"{}\">\
                            <input id=\"vec3-{prop_id}-y\" type=\"number\" step=\"{step}\" value=\"{}\">\
                            <input id=\"vec3-{prop_id}-z\" type=\"number\" step=\"{step}\" value=\"{}\">\
                            <button type=\"button\" onclick=\"sendVec3('{prop_id}')\">Set</button>\
                        </div>\
                    </div>",
                    initial.x, initial.y, initial.z
                );
            }
            PropertyControl::Button => {
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <button type=\"button\" onclick=\"sendUpdate('{prop_id}', '1')\">{label}</button>\
                    </div>"
                );
            }
        }
    }

    out.push_str(
        "<div class=\"control\">\
            <button type=\"button\" class=\"quit\" onclick=\"quitApp()\">Quit</button>\
        </div>",
    );
    out
}

/// Build the page shell and JavaScript event-posting helpers.
pub(crate) fn render_index_page(controls_html: &str) -> String {
    format!(
        "<!doctype html>\
        <html>\
        <head>\
            <meta charset=\"utf-8\">\
            <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
            <title>Tephrite Remote Control</title>\
            <style>\
                :root {{\
                    --bg-top: #f5fbff;\
                    --bg-bottom: #dce8f6;\
                    --panel: rgba(255, 255, 255, 0.9);\
                    --text: #143047;\
                    --muted: #5d7387;\
                    --accent: #0a7a88;\
                    --accent-2: #0f5f7e;\
                    --border: #c5d6e5;\
                    --danger: #8c1f1f;\
                    --danger-hover: #751616;\
                }}\
                * {{ box-sizing: border-box; }}\
                body {{\
                    margin: 0;\
                    min-height: 100vh;\
                    color: var(--text);\
                    font-family: 'Trebuchet MS', 'Gill Sans', 'Candara', sans-serif;\
                    background: radial-gradient(circle at 8% 0%, #ffffff 0%, var(--bg-top) 35%, var(--bg-bottom) 100%);\
                    display: grid;\
                    place-items: center;\
                    padding: 22px;\
                }}\
                .panel {{\
                    width: min(760px, 100%);\
                    background: var(--panel);\
                    border: 1px solid var(--border);\
                    border-radius: 18px;\
                    box-shadow: 0 18px 45px rgba(10, 30, 55, 0.12);\
                    overflow: hidden;\
                    animation: rise-in 260ms ease-out;\
                }}\
                .head {{\
                    padding: 20px 24px 14px;\
                    background: linear-gradient(120deg, rgba(8, 113, 126, 0.15), rgba(17, 74, 119, 0.11));\
                    border-bottom: 1px solid var(--border);\
                }}\
                h1 {{\
                    margin: 0;\
                    letter-spacing: 0.03em;\
                    font-size: clamp(1.3rem, 4vw, 1.8rem);\
                }}\
                .subtitle {{\
                    margin-top: 6px;\
                    font-size: 0.93rem;\
                    color: var(--muted);\
                }}\
                .controls {{\
                    padding: 14px 16px 20px;\
                }}\
                .control {{\
                    margin: 10px 0;\
                    padding: 12px;\
                    border: 1px solid var(--border);\
                    border-radius: 12px;\
                    background: rgba(255, 255, 255, 0.88);\
                }}\
                label {{\
                    font-weight: 600;\
                    color: #16364e;\
                }}\
                .value {{\
                    font-family: 'Consolas', 'Menlo', monospace;\
                    color: var(--accent-2);\
                }}\
                input, select, button {{\
                    font: inherit;\
                }}\
                input[type='range'] {{\
                    width: 100%;\
                    accent-color: var(--accent);\
                }}\
                input[type='text'],\
                input[type='number'],\
                select {{\
                    width: 100%;\
                    border: 1px solid var(--border);\
                    border-radius: 9px;\
                    padding: 8px 10px;\
                    color: var(--text);\
                    background: #fff;\
                }}\
                .vec3 {{\
                    display: grid;\
                    gap: 8px;\
                    grid-template-columns: repeat(3, minmax(0, 1fr)) auto;\
                    align-items: center;\
                }}\
                button {{\
                    border: 1px solid #0d6975;\
                    border-radius: 9px;\
                    padding: 8px 12px;\
                    background: linear-gradient(160deg, var(--accent), var(--accent-2));\
                    color: #fff;\
                    font-weight: 700;\
                    cursor: pointer;\
                    transition: transform 110ms ease, filter 110ms ease;\
                }}\
                button:hover {{\
                    filter: brightness(1.05);\
                    transform: translateY(-1px);\
                }}\
                .quit {{\
                    margin-top: 2px;\
                    background: var(--danger);\
                    border-color: var(--danger);\
                }}\
                .quit:hover {{\
                    background: var(--danger-hover);\
                }}\
                @media (max-width: 660px) {{\
                    body {{ padding: 12px; }}\
                    .panel {{ border-radius: 14px; }}\
                    .head {{ padding: 16px 16px 12px; }}\
                    .controls {{ padding: 10px 10px 14px; }}\
                    .vec3 {{ grid-template-columns: 1fr 1fr; }}\
                    .vec3 button {{ grid-column: 1 / -1; }}\
                }}\
                @keyframes rise-in {{\
                    from {{ opacity: 0; transform: translateY(10px); }}\
                    to {{ opacity: 1; transform: translateY(0); }}\
                }}\
            </style>\
        </head>\
        <body>\
            <main class=\"panel\">\
                <header class=\"head\">\
                    <h1>Remote Control</h1>\
                    <div class=\"subtitle\">Tephrite live parameters and triggers</div>\
                </header>\
                <section class=\"controls\">\
                    {controls_html}\
                </section>\
            </main>\
            <script>\
                async function sendUpdate(id, value) {{\
                    await fetch('{EVENT_PATH}', {{\
                        method: 'POST',\
                        headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},\
                        body: 'id=' + encodeURIComponent(id) + '&value=' + encodeURIComponent(value),\
                    }});\
                }}\
                function setLabel(id, value) {{\
                    const el = document.getElementById('value-' + id);\
                    if (el) el.innerText = value;\
                }}\
                function sendVec3(id) {{\
                    const x = document.getElementById('vec3-' + id + '-x')?.value ?? '0';\
                    const y = document.getElementById('vec3-' + id + '-y')?.value ?? '0';\
                    const z = document.getElementById('vec3-' + id + '-z')?.value ?? '0';\
                    sendUpdate(id, x + ',' + y + ',' + z);\
                }}\
                function quitApp() {{\
                    sendUpdate('{QUIT_ID}', '1');\
                }}\
            </script>\
        </body>\
        </html>"
    )
}
