import { EventType, EventRecord } from "../canonicalize/schema";

/**
 * EventCollector is responsible for hooking into Office.js events
 * and translating them into Proof Stamp EventRecords.
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
    // In a full implementation, we'd sync the context to find the exact paragraph and character offset
    // For now, we extract what's available from the event stream.
    
    // Determine event type
    let eventType = EventType.TYPED;
    // @ts-ignore - Some event properties are in preview/beta
    if (eventArgs.source === "Copilot" || eventArgs.source === "AI") {
        eventType = EventType.AI_INSERTION;
    } else if (eventArgs.type === "textDeleted") {
        eventType = EventType.DELETE;
    }

    const record: EventRecord = {
      id: crypto.randomUUID(),
      type: eventType,
      timestamp_unix_ms: Date.now(),
      paragraph_index: 0, // Requires complex range intersection tracking
      char_offset: 0,
      char_delta: eventType === EventType.DELETE ? -1 : 1,
    };

    // Filter duplicates within standard debouncing window.
    const last = this.events[this.events.length - 1];
    if (!last || last.timestamp_unix_ms < record.timestamp_unix_ms - 50 || last.type !== record.type) {
      this.events.push(record);
    }
  }
}
