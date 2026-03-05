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

/// Build the static control list HTML (plus auto-injected quit button).
pub(crate) fn render_controls(properties: &[PropertyDefinition]) -> String {
    let mut out = String::new();
    for property in properties {
        let prop_id = property.id.to_bits().to_string();
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
                body {{ font-family: sans-serif; margin: 20px; max-width: 680px; }}\
                h1 {{ margin-bottom: 16px; }}\
                .control {{ margin: 12px 0; }}\
                .value {{ font-family: monospace; }}\
                input[type='range'] {{ width: 100%; }}\
                .vec3 {{ display: flex; gap: 8px; align-items: center; }}\
                .vec3 input {{ width: 100px; }}\
                .quit {{ margin-top: 18px; background: #8c1f1f; color: #fff; border: none; padding: 10px 14px; cursor: pointer; }}\
            </style>\
        </head>\
        <body>\
            <h1>Remote Control</h1>\
            {controls_html}\
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
