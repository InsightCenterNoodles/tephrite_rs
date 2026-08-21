use crate::remote_control::common::*;
use crate::remote_control::property::*;

use std::fmt::Write as _;

const INDEX_TEMPLATE: &str = include_str!("assets/index.html");

pub(crate) const JQUERY_JS_PATH: &str = "/assets/jquery-1.7.1.min.js";
pub(crate) const JQUERY_TERMINAL_JS_PATH: &str = "/assets/jquery.terminal.min.js";
pub(crate) const JQUERY_TERMINAL_CSS_PATH: &str = "/assets/jquery.terminal.min.css";

pub(crate) const JQUERY_JS: &[u8] = include_bytes!("assets/vendor/jquery-1.7.1.min.js");
pub(crate) const JQUERY_TERMINAL_JS: &[u8] = include_bytes!("assets/vendor/jquery.terminal.min.js");
pub(crate) const JQUERY_TERMINAL_CSS: &[u8] =
    include_bytes!("assets/vendor/jquery.terminal.min.css");

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

/// Build the static control list HTML.
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
            PropertyControl::RangeSlider {
                min,
                max,
                step,
                initial_low,
                initial_high,
            } => {
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}: <span class=\"value\" id=\"value-{prop_id}\">{initial_low} - {initial_high}</span></label>\
                        <div class=\"dual-range-wrapper\">\
                            <div class=\"slider-track\" id=\"track-{prop_id}\"></div>\
                            <input type=\"range\" id=\"dual-{prop_id}-low\" min=\"{min}\" max=\"{max}\" step=\"{step}\" value=\"{initial_low}\" \
                                   oninput=\"updateDualRange('{prop_id}', {min}, {max})\" onchange=\"sendDualRange('{prop_id}')\">\
                            <input type=\"range\" id=\"dual-{prop_id}-high\" min=\"{min}\" max=\"{max}\" step=\"{step}\" value=\"{initial_high}\" \
                                   oninput=\"updateDualRange('{prop_id}', {min}, {max})\" onchange=\"sendDualRange('{prop_id}')\">\
                        </div>\
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
            PropertyControl::Analog { initial } => {
                let _ = write!(
                    out,
                    "<div class=\"control\">\
                        <label>{label}: <span class=\"value\" id=\"analog-value-{prop_id}\">{:.2}, {:.2}</span></label>\
                        <div class=\"analog\" id=\"analog-{prop_id}\" data-prop-id=\"{prop_id}\" data-x=\"{}\" data-y=\"{}\" tabindex=\"0\" role=\"slider\" aria-label=\"{label}\" aria-valuemin=\"-1\" aria-valuemax=\"1\" aria-valuetext=\"{:.2}, {:.2}\">\
                            <div class=\"analog-cross\"></div>\
                            <div class=\"analog-stick\" id=\"analog-stick-{prop_id}\"></div>\
                        </div>\
                    </div>",
                    initial.x, initial.y, initial.x, initial.y, initial.x, initial.y
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

    out
}

/// Build the page shell and JavaScript helpers from the checked-in template.
pub(crate) fn render_index_page(controls_html: &str, brp_port: Option<u16>) -> String {
    let brp_url = brp_port
        .map(|port| format!("'http://' + window.location.hostname + ':{port}/'"))
        .unwrap_or_else(|| "null".into());

    INDEX_TEMPLATE
        .replace("{{CONTROLS_HTML}}", controls_html)
        .replace("__BRP_URL__", &brp_url)
        .replace("__EVENT_PATH__", EVENT_PATH)
        .replace("__API_ENTITIES_PATH__", API_ENTITIES_PATH)
        .replace("__API_TRANSFORM_LOOK_AT_PATH__", API_TRANSFORM_LOOK_AT_PATH)
        .replace("__API_DEBUG_ENABLE_PATH__", API_DEBUG_ENABLE_PATH)
        .replace("__QUIT_ID__", QUIT_ID)
}
