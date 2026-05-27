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
                .dual-range-wrapper {{\
                    position: relative;\
                    width: 100%;\
                    height: 24px;\
                    margin-top: 10px;\
                    display: flex;\
                    align-items: center;\
                }}\
                .slider-track {{\
                    width: 100%;\
                    height: 6px;\
                    position: absolute;\
                    background: var(--border);\
                    border-radius: 3px;\
                }}\
                .dual-range-wrapper input[type='range'] {{\
                    position: absolute;\
                    width: 100%;\
                    background: none;\
                    pointer-events: none;\
                    -webkit-appearance: none;\
                    -moz-appearance: none;\
                    appearance: none;\
                    margin: 0;\
                }}\
                .dual-range-wrapper input[type='range']::-webkit-slider-thumb {{\
                    height: 18px;\
                    width: 18px;\
                    border-radius: 50%;\
                    background-color: var(--accent);\
                    cursor: pointer;\
                    pointer-events: auto;\
                    -webkit-appearance: none;\
                    box-shadow: 0 1px 4px rgba(0,0,0,0.3);\
                }}\
                .dual-range-wrapper input[type='range']::-moz-range-thumb {{\
                    height: 18px;\
                    width: 18px;\
                    border: none;\
                    border-radius: 50%;\
                    background-color: var(--accent);\
                    cursor: pointer;\
                    pointer-events: auto;\
                    box-shadow: 0 1px 4px rgba(0,0,0,0.3);\
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
                .analog {{\
                    position: relative;\
                    width: min(240px, 100%);\
                    aspect-ratio: 1 / 1;\
                    margin-top: 10px;\
                    border-radius: 999px;\
                    border: 2px solid #8ea6bb;\
                    background: radial-gradient(circle at 30% 28%, #ffffff, #d6e4f0);\
                    touch-action: none;\
                    outline: none;\
                }}\
                .analog:focus-visible {{\
                    box-shadow: 0 0 0 3px rgba(10, 122, 136, 0.35);\
                }}\
                .analog-cross {{\
                    position: absolute;\
                    inset: 0;\
                    border-radius: 999px;\
                    background:\
                        linear-gradient(to right, transparent 49.2%, rgba(35, 68, 93, 0.25) 49.2%, rgba(35, 68, 93, 0.25) 50.8%, transparent 50.8%),\
                        linear-gradient(to bottom, transparent 49.2%, rgba(35, 68, 93, 0.25) 49.2%, rgba(35, 68, 93, 0.25) 50.8%, transparent 50.8%);\
                    pointer-events: none;\
                }}\
                .analog-stick {{\
                    position: absolute;\
                    width: 34px;\
                    height: 34px;\
                    border-radius: 999px;\
                    left: 50%;\
                    top: 50%;\
                    transform: translate(-50%, -50%);\
                    background: linear-gradient(160deg, #0a7a88, #0f5f7e);\
                    border: 2px solid #ffffff;\
                    box-shadow: 0 4px 16px rgba(7, 30, 48, 0.25);\
                    pointer-events: none;\
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
                                function updateDualRange(id, min, max) {{\
                    const low = document.getElementById('dual-' + id + '-low');\
                    const high = document.getElementById('dual-' + id + '-high');\
                    const track = document.getElementById('track-' + id);\
                    if (!low || !high || !track) return;\
                    \
                    let lowVal = parseFloat(low.value);\
                    let highVal = parseFloat(high.value);\
                    \
                    if (lowVal > highVal) {{\
                        if (document.activeElement === low) {{ low.value = highVal; lowVal = highVal; }}\
                        else {{ high.value = lowVal; highVal = lowVal; }}\
                    }}\
                    \
                    setLabel(id, lowVal + ' - ' + highVal);\
                    \
                    const percent1 = ((lowVal - min) / (max - min)) * 100;\
                    const percent2 = ((highVal - min) / (max - min)) * 100;\
                    track.style.background = 'linear-gradient(to right, var(--border) ' + percent1 + '%, var(--accent) ' + percent1 + '%, var(--accent) ' + percent2 + '%, var(--border) ' + percent2 + '%)';\
                }}\
                function sendDualRange(id) {{\
                    const low = document.getElementById('dual-' + id + '-low');\
                    const high = document.getElementById('dual-' + id + '-high');\
                    if (low && high) sendUpdate(id, low.value + ',' + high.value);\
                }}\
                function sendVec3(id) {{\
                    const x = document.getElementById('vec3-' + id + '-x')?.value ?? '0';\
                    const y = document.getElementById('vec3-' + id + '-y')?.value ?? '0';\
                    const z = document.getElementById('vec3-' + id + '-z')?.value ?? '0';\
                    sendUpdate(id, x + ',' + y + ',' + z);\
                }}\
                function clampAnalog(v) {{\
                    return Math.max(-1, Math.min(1, v));\
                }}\
                function analogValueToString(x, y) {{\
                    return x.toFixed(3) + ',' + y.toFixed(3);\
                }}\
                function setAnalogVisual(id, x, y) {{\
                    const stick = document.getElementById('analog-stick-' + id);\
                    const root = document.getElementById('analog-' + id);\
                    const label = document.getElementById('analog-value-' + id);\
                    if (!stick || !root) return;\
                    stick.style.left = (50 + x * 40) + '%';\
                    stick.style.top = (50 - y * 40) + '%';\
                    if (label) label.innerText = x.toFixed(2) + ', ' + y.toFixed(2);\
                    root.setAttribute('aria-valuetext', x.toFixed(2) + ', ' + y.toFixed(2));\
                }}\
                function setupAnalog(root) {{\
                    const id = root.dataset.propId;\
                    if (!id) return;\
                    let x = clampAnalog(parseFloat(root.dataset.x ?? '0') || 0);\
                    let y = clampAnalog(parseFloat(root.dataset.y ?? '0') || 0);\
                    let pointerId = null;\
                    let activeTimer = null;\
                    let keyboardTimer = null;\
                    const keyboardState = {{ left: false, right: false, up: false, down: false }};\
\
                    function sendNow() {{\
                        sendUpdate(id, analogValueToString(x, y));\
                    }}\
                    function ensureActiveSendLoop() {{\
                        if (activeTimer !== null) return;\
                        activeTimer = setInterval(() => {{\
                            if (Math.abs(x) < 0.0001 && Math.abs(y) < 0.0001) {{\
                                clearInterval(activeTimer);\
                                activeTimer = null;\
                                return;\
                            }}\
                            sendNow();\
                        }}, 80);\
                    }}\
                    function stopActiveSendLoop() {{\
                        if (activeTimer !== null) {{\
                            clearInterval(activeTimer);\
                            activeTimer = null;\
                        }}\
                    }}\
                    function updateFromClientPoint(clientX, clientY) {{\
                        const rect = root.getBoundingClientRect();\
                        const cx = rect.left + rect.width * 0.5;\
                        const cy = rect.top + rect.height * 0.5;\
                        const dx = (clientX - cx) / (rect.width * 0.5);\
                        const dy = (cy - clientY) / (rect.height * 0.5);\
                        const len = Math.hypot(dx, dy);\
                        if (len > 1) {{\
                            x = dx / len;\
                            y = dy / len;\
                        }} else {{\
                            x = dx;\
                            y = dy;\
                        }}\
                        x = clampAnalog(x);\
                        y = clampAnalog(y);\
                        setAnalogVisual(id, x, y);\
                        sendNow();\
                        if (x !== 0 || y !== 0) ensureActiveSendLoop();\
                    }}\
                    function resetToCenter(send) {{\
                        x = 0;\
                        y = 0;\
                        setAnalogVisual(id, x, y);\
                        stopActiveSendLoop();\
                        if (send) sendNow();\
                    }}\
                    function updateFromKeyboardState() {{\
                        const tx = (keyboardState.right ? 1 : 0) - (keyboardState.left ? 1 : 0);\
                        const ty = (keyboardState.up ? 1 : 0) - (keyboardState.down ? 1 : 0);\
                        x = clampAnalog(tx);\
                        y = clampAnalog(ty);\
                        setAnalogVisual(id, x, y);\
                        sendNow();\
                        if (x !== 0 || y !== 0) ensureActiveSendLoop();\
                    }}\
\
                    root.addEventListener('pointerdown', (e) => {{\
                        pointerId = e.pointerId;\
                        root.setPointerCapture(pointerId);\
                        updateFromClientPoint(e.clientX, e.clientY);\
                        root.focus();\
                    }});\
                    root.addEventListener('pointermove', (e) => {{\
                        if (pointerId !== e.pointerId) return;\
                        updateFromClientPoint(e.clientX, e.clientY);\
                    }});\
                    function finishPointer(e) {{\
                        if (pointerId !== e.pointerId) return;\
                        pointerId = null;\
                        resetToCenter(true);\
                    }}\
                    root.addEventListener('pointerup', finishPointer);\
                    root.addEventListener('pointercancel', finishPointer);\
\
                    root.addEventListener('keydown', (e) => {{\
                        let consumed = true;\
                        if (e.key === 'ArrowLeft' || e.key === 'a' || e.key === 'A') keyboardState.left = true;\
                        else if (e.key === 'ArrowRight' || e.key === 'd' || e.key === 'D') keyboardState.right = true;\
                        else if (e.key === 'ArrowUp' || e.key === 'w' || e.key === 'W') keyboardState.up = true;\
                        else if (e.key === 'ArrowDown' || e.key === 's' || e.key === 'S') keyboardState.down = true;\
                        else consumed = false;\
                        if (!consumed) return;\
                        e.preventDefault();\
                        updateFromKeyboardState();\
                        if (keyboardTimer === null) {{\
                            keyboardTimer = setInterval(updateFromKeyboardState, 90);\
                        }}\
                    }});\
                    root.addEventListener('keyup', (e) => {{\
                        let consumed = true;\
                        if (e.key === 'ArrowLeft' || e.key === 'a' || e.key === 'A') keyboardState.left = false;\
                        else if (e.key === 'ArrowRight' || e.key === 'd' || e.key === 'D') keyboardState.right = false;\
                        else if (e.key === 'ArrowUp' || e.key === 'w' || e.key === 'W') keyboardState.up = false;\
                        else if (e.key === 'ArrowDown' || e.key === 's' || e.key === 'S') keyboardState.down = false;\
                        else consumed = false;\
                        if (!consumed) return;\
                        e.preventDefault();\
                        const anyDown = keyboardState.left || keyboardState.right || keyboardState.up || keyboardState.down;\
                        if (anyDown) {{\
                            updateFromKeyboardState();\
                        }} else {{\
                            if (keyboardTimer !== null) {{\
                                clearInterval(keyboardTimer);\
                                keyboardTimer = null;\
                            }}\
                            resetToCenter(true);\
                        }}\
                    }});\
                    root.addEventListener('blur', () => {{\
                        keyboardState.left = false;\
                        keyboardState.right = false;\
                        keyboardState.up = false;\
                        keyboardState.down = false;\
                        if (keyboardTimer !== null) {{\
                            clearInterval(keyboardTimer);\
                            keyboardTimer = null;\
                        }}\
                        resetToCenter(true);\
                    }});\
\
                    setAnalogVisual(id, x, y);\
                    if (x !== 0 || y !== 0) ensureActiveSendLoop();\
                }}\
                window.addEventListener('DOMContentLoaded', () => {{\
                    document.querySelectorAll('.analog[data-prop-id]').forEach(setupAnalog);\
                }});\
                function quitApp() {{\
                    sendUpdate('{QUIT_ID}', '1');\
                }}\
            </script>\
        </body>\
        </html>"
    )
}
