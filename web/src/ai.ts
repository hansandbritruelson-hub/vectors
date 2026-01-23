import { CreateMLCEngine, MLCEngine } from "@mlc-ai/web-llm";
import { removeBackground, type Config } from "@imgly/background-removal";

const SELECTED_MODEL = "Mistral-7B-Instruct-v0.3-q4f16_1-MLC";

export interface AICommand {
  action: string;
  params: any;
}

export interface ModelStatus {
  status: "idle" | "loading" | "ready" | "error";
  message: string;
  progress?: number;
}

const SYSTEM_PROMPT = "You are a vector graphics assistant. You control a vector engine via JSON commands. " +
"Respond ONLY with a JSON ARRAY of commands. " +
"Example: [{\"action\": \"add\", \"params\": {...}}] " +
"Commands: " +
"1. { \"action\": \"add\", \"params\": { \"type\": \"Rectangle\" | \"Circle\", \"x\": number, \"y\": number, \"width\": number, \"height\": number, \"fill\": string } } " +
"2. { \"action\": \"update\", \"params\": { \"id\": number, \"x\"?: number, \"y\"?: number, \"width\"?: number, \"height\"?: number, \"fill\"?: string, \"stroke\"?: string, \"stroke_width\"?: number, \"opacity\"?: number, \"name\"?: string } } " +
"3. { \"action\": \"delete\", \"params\": { \"id\": number } } " +
"4. { \"action\": \"select\", \"params\": { \"id\": number } } " +
"Rules: " +
"- Return ONLY the JSON array. " +
"- Use 'update' to move objects or change their appearance. " +
"- Object IDs are provided in the CURRENT STATE. " +
"- (x, y) are the coordinates of the top-left corner. " +
"- If you want to perform one action, wrap it in []. Do not include markdown code blocks.";

class AIService {
  private engine: MLCEngine | null = null;
  private initPromise: Promise<void> | null = null;
  public onStatusUpdate: ((status: ModelStatus) => void) | null = null;

  async init() {
    if (this.engine) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = (async () => {
      try {
        this.updateStatus("loading", "Initializing WebGPU...");
        this.engine = await CreateMLCEngine(SELECTED_MODEL, {
          initProgressCallback: (report) => {
            this.updateStatus("loading", report.text, report.progress);
          },
          logLevel: "INFO",
        });
        this.updateStatus("ready", "Model loaded and ready.");
      } catch (error: any) {
        console.error("AIService Init Error:", error);
        this.updateStatus("error", "Failed to load: " + error.message);
        this.initPromise = null;
        throw error;
      }
    })();

    return this.initPromise;
  }

  private updateStatus(status: ModelStatus['status'], message: string, progress?: number) {
    if (this.onStatusUpdate) this.onStatusUpdate({ status, message, progress });
  }

  async removeBackground(imageSource: string | URL | HTMLImageElement | HTMLCanvasElement | Blob): Promise<Blob> {
    console.log("Starting background removal with source:", imageSource);
    this.updateStatus("loading", "Removing background...");
    try {
      const config: Config = {
        debug: true,
        device: 'cpu',
        progress: (status, progress) => {
          // Normalize progress: some stages might return absolute values or different scales
          // Usually imgly returns 0-1, but let's be safe.
          const normalizedProgress = progress > 1 ? (progress / 100) : progress;
          
          let friendlyStatus = status;
          if (status.includes('fetch')) friendlyStatus = 'Downloading AI model...';
          if (status.includes('compute')) friendlyStatus = 'AI is processing image...';
          if (status.includes('complete')) friendlyStatus = 'Finishing up...';

          console.log(`BG Removal Progress: ${status} (${progress}) -> ${friendlyStatus}`);
          this.updateStatus("loading", friendlyStatus, normalizedProgress);
        }
      };
      const resultBlob = await removeBackground(imageSource, config);
      this.updateStatus("ready", "Background removed successfully.");
      return resultBlob;
    } catch (error: any) {
      console.error("Background Removal Error:", error);
      console.error("Stack trace:", error.stack);
      this.updateStatus("error", "Failed to remove background: " + error.message);
      throw error;
    }
  }

  async processPrompt(userPrompt: string, objectsJson: string): Promise<AICommand[]> {
    await this.init();
    if (!this.engine) throw new Error("AI Engine not available");

    const messages = [
      { role: "system" as const, content: SYSTEM_PROMPT },
      { role: "user" as const, content: "CURRENT STATE: " + objectsJson + "\nREQUEST: " + userPrompt }
    ];

    try {
      // @ts-ignore
      const reply = await this.engine.chat.completions.create({
        messages,
        temperature: 0.1,
        max_tokens: 1024
      });
      
      const content = reply.choices[0].message.content || "";
      console.log("AI Raw Response:", content);
      return this.postProcessResponse(content);
    } catch (error) {
      console.error("AI Process Error:", error);
      return [];
    }
  }

  private postProcessResponse(text: string): AICommand[] {
    text = text.trim();
    
    // Attempt 1: Direct JSON parse
    try {
      const parsed = JSON.parse(text);
      return this.ensureArray(parsed);
    } catch (e) {
      // Attempt 2: Extract array or object using regex
      const arrayMatch = text.match(/[[\]\s*\{*.*\}*\s*\]]/s);
      if (arrayMatch) {
        try {
          return JSON.parse(arrayMatch[0]);
        } catch (e2) {}
      }

      const objectMatch = text.match(/\{\s*".*"\s*:\s*.*\}/s);
      if (objectMatch) {
        try {
          const parsed = JSON.parse(objectMatch[0]);
          return this.ensureArray(parsed);
        } catch (e3) {}
      }
    }
    
    console.warn("AIService: No valid JSON found in response.");
    return [];
  }

  private ensureArray(parsed: any): AICommand[] {
    if (Array.isArray(parsed)) return parsed;
    if (parsed && typeof parsed === 'object') {
      // If it's a single command object
      if (parsed.action) return [parsed as AICommand];
      // If it's an object containing an array (e.g., { "commands": [...] })
      if (parsed.commands && Array.isArray(parsed.commands)) return parsed.commands;
    }
    return [];
  }
}

export const aiService = new AIService();
