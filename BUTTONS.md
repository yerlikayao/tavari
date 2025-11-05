# WhatsApp Interactive Buttons - Future Implementation

## Current Status

WhatsApp interactive quick reply buttons are **currently disabled** because Bird.com API requires using **WhatsApp Template Messages** for button functionality.

## Why Buttons Don't Work

Bird.com API error:
```
Bird.com API error (422 Unprocessable Entity): {
  "code": "InvalidPayload",
  "message": "One or more fields provided in the request body are malformed",
  "details": {
    ".body.type": [
      "Invalid value: \"interactive\", expected one of: \"text\", \"html\", \"image\", \"file\", \"gif\", \"location\", \"carousel\", \"list\", \"section\", \"authentication\", \"template\", \"action\""
    ]
  }
}
```

## How to Enable Buttons (Future)

### Step 1: Create WhatsApp Template Messages

1. Go to **WhatsApp Business Manager** (https://business.facebook.com/)
2. Navigate to **WhatsApp Manager** → **Message Templates**
3. Create a new template with quick reply buttons
4. Wait for Facebook approval (can take 24-48 hours)

### Step 2: Template Example

**Template Name:** `water_logging_prompt`

**Template Content:**
```
Hızlı su kaydı:
Bugünkü toplam: {{1}} ml
Hedef: {{2}} ml
```

**Quick Reply Buttons:**
- Button 1: "💧 150 ml"
- Button 2: "💧 250 ml"
- Button 3: "💧 500 ml"

### Step 3: Update Code

Replace `send_message()` calls with `send_template_message()` using approved template:

```rust
// Example template message format for Bird.com
let template_request = json!({
    "receiver": {
        "contacts": [{
            "identifierValue": phone_number
        }]
    },
    "body": {
        "type": "template",
        "template": {
            "namespace": "YOUR_NAMESPACE_ID",
            "name": "water_logging_prompt",
            "language": {
                "code": "tr"
            },
            "components": [{
                "type": "body",
                "parameters": [
                    { "type": "text", "text": "1500" },
                    { "type": "text", "text": "2000" }
                ]
            }]
        }
    }
});
```

### Step 4: Handle Button Clicks

Button clicks are received as regular messages with the button text. The code already handles this:

```rust
// In message_handler.rs
if message_lower.starts_with("water_") {
    self.handle_water_button(from, message).await?;
    return Ok(());
}
```

## Alternative: List Messages

Bird.com also supports **list messages** which don't require template approval:

```json
{
  "body": {
    "type": "list",
    "list": {
      "header": "Su Kayıt",
      "body": "Hızlı kayıt için seçin:",
      "footer": "Tavari Bot",
      "sections": [{
        "title": "Miktarlar",
        "rows": [
          {"id": "water_150", "title": "150 ml"},
          {"id": "water_250", "title": "250 ml"},
          {"id": "water_500", "title": "500 ml"}
        ]
      }]
    }
  }
}
```

## References

- [Bird.com WhatsApp Template Messages](https://docs.bird.com/connectivity-platform/use-cases/use-buttons-to-create-interactive-whatsapp-template-messages)
- [MessageBird Interactive Messages](https://developers.messagebird.com/quickstarts/whatsapp/send-interactive-messages/)
- [WhatsApp Business Manager](https://business.facebook.com/wa/manage/message-templates/)

## Code Location

The button implementation code is preserved in:
- `src/services/bird.rs` - `send_message_with_buttons()` method (marked with `#[allow(dead_code)]`)
- `src/handlers/message_handler.rs` - Button click handlers (still active for future use)
