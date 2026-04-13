#!/bin/bash
# MDM Desktop App Icon Generator via BizRouter → Gemini Pro 3
# Generates 10 icon concepts in parallel

API_KEY="sk-br-v1-71f7a25b1f3540678c3a04336827fd00_JAALMr1SDjwGwlSscWhTpi3S6YTK1ZrVAvwQNv-vHkQ"
BASE_URL="https://api.bizrouter.ai/v1/chat/completions"
MODEL="google/gemini-3-pro-image-preview"
OUTPUT_DIR="/Users/seunghan/markdown-media/desktop/icon-concepts"

generate_icon() {
  local num="$1"
  local style="$2"
  local prompt="$3"

  echo "[$num] Generating: $style..."

  local response
  response=$(curl -s -X POST "$BASE_URL" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    -H "X-Title: MDM-Icon-Gen" \
    --max-time 120 \
    -d "{
      \"model\": \"$MODEL\",
      \"messages\": [{
        \"role\": \"user\",
        \"content\": \"$prompt\"
      }],
      \"aspect_ratio\": \"1:1\",
      \"image_size\": \"2K\"
    }")

  # Extract base64 image from response
  local image_data
  image_data=$(echo "$response" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    choices = data.get('choices', [])
    if choices:
        msg = choices[0].get('message', {})
        content = msg.get('content', '')
        if isinstance(content, list):
            for part in content:
                if part.get('type') == 'image_url':
                    url = part['image_url']['url']
                    # Remove data:image/png;base64, prefix
                    print(url.split(',', 1)[1] if ',' in url else url)
                    break
        else:
            print('TEXT_ONLY', file=sys.stderr)
    else:
        err = data.get('error', {})
        print(f'ERROR: {err}', file=sys.stderr)
except Exception as e:
    print(f'PARSE_ERROR: {e}', file=sys.stderr)
" 2>"$OUTPUT_DIR/${num}_${style}.err")

  if [ -n "$image_data" ] && [ "$image_data" != "" ]; then
    echo "$image_data" | base64 -d > "$OUTPUT_DIR/${num}_${style}.png" 2>/dev/null
    if [ -s "$OUTPUT_DIR/${num}_${style}.png" ]; then
      echo "[$num] ✅ Saved: ${num}_${style}.png ($(wc -c < "$OUTPUT_DIR/${num}_${style}.png") bytes)"
    else
      echo "[$num] ❌ Failed to decode base64 for $style"
      echo "$response" | head -c 500 > "$OUTPUT_DIR/${num}_${style}_debug.json"
    fi
  else
    echo "[$num] ❌ No image in response for $style"
    echo "$response" | head -c 1000 > "$OUTPUT_DIR/${num}_${style}_debug.json"
  fi
}

# Common context for all prompts
CTX="Design a desktop application icon for 'MDM' (Markdown-Media), a high-performance document converter engine built in Rust. It converts HWP, PDF, DOCX files into clean Markdown for AI/LLM consumption. The icon should work as macOS and Windows app icon at all sizes from 16x16 to 1024x1024. NO text in the icon. The icon should be inside a rounded rectangle (macOS squircle shape) with solid or gradient background."

# Generate 10 variants in parallel
generate_icon "01" "dynamic-minimalism" "${CTX} Style: Dynamic Minimalism 2025. Single bold geometric symbol representing document-to-markdown transformation. Use an abstract arrow or flow shape showing conversion. Colors: deep indigo (#1a1a2e) to electric blue (#0066ff). Ultra clean, no details, one strong shape. Think of how Notion or Linear app icons use minimal geometry." &

generate_icon "02" "liquid-glass" "${CTX} Style: Apple Liquid Glass 2025 (WWDC25). Translucent glass-like material with edge highlights, frostiness, and light refraction. Show a stylized document page morphing into markdown hash symbol through the glass. Soft light-to-dark gradient background. The icon should look like it's lit from within. Subtle chromatic aberration on edges." &

generate_icon "03" "vibrant-gradient" "${CTX} Style: Vibrant Aurora Gradient 2025. Mesh gradient background flowing from coral (#FF6B6B) through purple (#845EC2) to cyan (#00C9A7). Center element: an abstract document shape with flowing lines suggesting text being transformed. Modern, eye-catching, high contrast. Similar energy to the Instagram or Firefox gradient style." &

generate_icon "04" "3d-clay" "${CTX} Style: 3D Clay / Soft Render 2026. Soft matte clay material, rounded edges, subtle shadows. Show a cute 3D document icon with a small arrow transforming into a markdown hash. Warm pastel background. Friendly, approachable, almost touchable. Think Figma or Craft app icon style. Consistent top-left light source." &

generate_icon "05" "neo-brutalism" "${CTX} Style: Neo-Brutalism 2025. Thick 4px black borders, bold flat colors, harsh drop shadow offset to bottom-right. Bright yellow (#FFD700) background with a black document shape and a bold red arrow pointing to a markdown symbol. Raw, bold, intentionally unrefined but modern. High contrast. Think Gumroad or Notion alternate style." &

wait

generate_icon "06" "metallic-premium" "${CTX} Style: Metallic Premium 2025. Polished chrome/brushed steel finish with subtle reflections. A sophisticated abstract M symbol that suggests both 'Markdown' and 'Media'. Dark background (#1a1a1a) with metallic silver/platinum element. Premium, professional, developer-tool aesthetic. Similar to Tower Git or Proxyman app icons." &

generate_icon "07" "line-art-minimal" "${CTX} Style: Minimalist Line Art 2025. Single consistent line weight (2px equivalent). White or very light gray background. A continuous line drawing that forms both a document page and flows into a markdown hash symbol. Elegant, intellectual, timeless. Single accent color: electric blue (#0066ff). Think Bear app or iA Writer icon simplicity." &

generate_icon "08" "glassmorphism" "${CTX} Style: Glassmorphism 2.0 2025. Frosted glass panel with blur backdrop, subtle white border, and depth. Behind the glass: colorful abstract shapes suggesting documents. On the glass surface: a clean white arrow or flow symbol. Background gradient from deep purple to dark blue. Modern, sophisticated depth effect." &

generate_icon "09" "dark-mode-first" "${CTX} Style: Dark Mode First 2026. Designed primarily for dark backgrounds. Near-black background (#0d0d0d) with a glowing neon accent. The main element is a document-to-markdown flow symbol that appears to emit soft light. Accent color: electric cyan (#00E5FF) with subtle glow/bloom effect. Minimal, futuristic, developer-focused. Think Terminal or Warp app icons." &

generate_icon "10" "gummi-soft3d" "${CTX} Style: Soft 3D Gummi 2026 (hottest trend). Rounded organic shapes, translucent quality, gentle gradients. A playful yet professional document icon with soft inflated 3D appearance like a gummy bear texture. Colors: soft gradient from lavender (#E8D5FF) through sky blue (#87CEEB) to mint (#98FB98). Subtle inner glow, no harsh edges. Modern, warm, innovative." &

wait

echo ""
echo "=== Generation Complete ==="
ls -la "$OUTPUT_DIR"/*.png 2>/dev/null | awk '{print $5, $9}' | column -t
echo ""
echo "Error logs:"
for f in "$OUTPUT_DIR"/*.err; do
  [ -s "$f" ] && echo "  $(basename $f): $(cat $f)"
done
