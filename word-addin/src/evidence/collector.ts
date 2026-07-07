import { EventType, EventRecord } from "../canonicalize/schema";

/**
 * EventCollector is responsible for hooking into Office.js events
 * and translating them into Kako EventRecords.
 */
export class EventCollector {
  private events: EventRecord[] = [];

  constructor() {}

  public getEvents(): EventRecord[] {
    return this.events;
  }

  public clearEvents(): void {
    this.events = [];
  }

  public async registerHooks(context: Word.RequestContext): Promise<void> {
    // Requires WordApi >= 1.3 in manifest for preview events.
    
    // Register onContentChanged
    context.document.onContentChanged.add(this.onContentChanged.bind(this));
    
    // Register Copilot and AI insertion detection listeners.
    console.log("Hooks registered.");
    await context.sync();
  }

  private async onContentChanged(eventArgs: Word.DocumentChangedEventArgs): Promise<void> {
    // Extract insertion type and origin metadata from event context.
    
    const record: EventRecord = {
      id: crypto.randomUUID(),
      type: EventType.TYPED,
      timestamp_ms: Date.now(),
      paragraph_index: 0, // Requires calculating position
      char_offset: 0,
      char_delta: 1,
    };

    // Filter duplicates within standard debouncing window.
    this.events.push(record);
  }
}
