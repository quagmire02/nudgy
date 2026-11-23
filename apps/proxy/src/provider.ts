// Where lines come from. One interface, so the model can change without
// touching the rest (the spec: "behind a provider interface").

export interface LineProvider {
  /** The model's raw reply to `prompt`. Throws on failure or timeout. */
  generate(prompt: string): Promise<string>;
}

const TIMEOUT_MS = 2500;

/** Google Gemini (Flash-Lite class by default). */
export class GeminiProvider implements LineProvider {
  private apiKey: string;
  private model: string;
  constructor(apiKey: string, model: string) {
    this.apiKey = apiKey;
    this.model = model;
  }

  async generate(prompt: string): Promise<string> {
    const url = `https://generativelanguage.googleapis.com/v1beta/models/${encodeURIComponent(this.model)}:generateContent`;
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-goog-api-key": this.apiKey },
      body: JSON.stringify({
        contents: [{ role: "user", parts: [{ text: prompt }] }],
        generationConfig: { maxOutputTokens: 60, temperature: 0.9 },
      }),
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    if (!res.ok) throw new Error(`gemini ${res.status}`);
    const data = (await res.json()) as { candidates?: { content?: { parts?: { text?: string }[] } }[] };
    const text = data.candidates?.[0]?.content?.parts?.map((p) => p.text ?? "").join("") ?? "";
    if (!text.trim()) throw new Error("gemini returned no text");
    return text;
  }
}

/** Fixed replies, for tests. */
export class FakeProvider implements LineProvider {
  prompts: string[] = [];
  private reply: string | Error;
  constructor(reply: string | Error = "Six minutes of Instagram. Chapter 3 misses you.") {
    this.reply = reply;
  }
  async generate(prompt: string): Promise<string> {
    this.prompts.push(prompt);
    if (this.reply instanceof Error) throw this.reply;
    return this.reply;
  }
}
