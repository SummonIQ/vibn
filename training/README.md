# Fine-tuning Vibn's Local Model

Fine-tune `Qwen2.5-Coder-7B-Instruct` on your own Vibn sessions so the model learns your codebase, your style, and how to use Vibn's tools correctly.

---

## How it works

Every conversation you have with Vibn is saved to `~/.vibn/transcripts/`. This pipeline converts those sessions into a training dataset and fine-tunes the model using **QLoRA** — a technique that trains only a small set of adapter weights on top of the frozen base model. The result is a GGUF file you load directly in Ollama.

```
~/.vibn/transcripts/*.jsonl
         │
         ▼
/export-training-data     ← filters + converts to ShareGPT format
         │
         ▼
vibn_training_data.jsonl  ← upload this to Colab
         │
         ▼
vibn_finetune.ipynb       ← runs on free T4 GPU (~45 min)
         │
         ▼
vibn-coder-q4.gguf        ← download and load in Ollama
         │
         ▼
./vibn -m vibn-coder
```

---

## Step 1 — Accumulate good sessions

Use Vibn normally for a week or two. The more tool-using sessions you have, the better. Sessions where the agent:
- reads files before editing them
- runs tests to verify changes
- uses `search_code` to find things
- completes multi-step tasks

...are the most valuable training examples.

**Aim for:** 50–200 sessions with at least 3 user turns each.

---

## Step 2 — Export training data

```bash
cd /Users/steven/Projects/llm

# See what you have:
#   ./vibn
#   /export-training-data --stats

# Export from inside Vibn:
#   /export-training-data

# Stricter export:
#   /export-training-data --require-tools --min-turns 3

# Output goes to ~/vibn_training_data.jsonl by default
```

Or from inside Vibn: `/export-training-data`

---

## Step 3 — Fine-tune on Google Colab

1. Go to [colab.research.google.com](https://colab.research.google.com)
2. **File → Upload notebook** → select `training/vibn_finetune.ipynb`
3. **Runtime → Change runtime type → T4 GPU** (free tier is fine)
4. Run all cells top to bottom — each cell has instructions
5. When prompted, upload `~/vibn_training_data.jsonl`
6. At the end, download the `.gguf` file (~4.5 GB)

**Expected time:**
| GPU | Time | Cost |
|-----|------|------|
| T4 (free Colab) | ~45 min | Free |
| A100 (Colab Pro) | ~15 min | ~$0.50 |
| H100 (RunPod) | ~8 min | ~$0.30 |

---

## Step 4 — Load in Ollama

```bash
# Put the downloaded GGUF where Ollama can find it
mkdir -p ~/models
mv ~/Downloads/vibn-coder-unsloth.Q4_K_M.gguf ~/models/

# Register with Ollama using the Modelfile
ollama create vibn-coder -f /Users/steven/Projects/llm/training/Modelfile

# Verify it's there
ollama list

# Run Vibn with your fine-tuned model
cd /Users/steven/Projects/llm
./vibn -m vibn-coder

# Or set it as default
# Edit ~/.vibn/config.json → "default_model": "vibn-coder"
```

---

## Tips

**More data = better results.** 50 sessions is enough to see improvement. 500 is noticeably better.

**Quality over quantity.** One session where the agent correctly reads → edits → tests is worth 10 sessions where it just chats.

**Re-train periodically.** After another month of sessions, re-export and re-train. The model compounds your usage patterns.

**The Modelfile matters.** Edit `training/Modelfile` to adjust the system prompt, context window (`num_ctx`), and temperature. A lower temperature (0.1–0.2) makes the model more deterministic and tool-focused.

**If the model regresses**, lower `EPOCHS` from 3 to 1–2. Overfitting on small datasets makes the model repetitive.

---

## File reference

| File | Purpose |
|------|---------|
| `/export-training-data` | Rust-native slash command that converts `~/.vibn/transcripts/` to ShareGPT JSONL |
| `training/vibn_finetune.ipynb` | Colab notebook — loads data, trains, exports GGUF |
| `training/Modelfile` | Ollama model definition for the fine-tuned model |
| `training/README.md` | This file |
