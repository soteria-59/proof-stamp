import { EventCollector } from "../evidence/collector";
import { onBeforeSave } from "../stamp/save-interceptor";

let collector: EventCollector;

Office.onReady((info) => {
  if (info.host === Office.HostType.Word) {
    document.getElementById("status-indicator")!.innerText = "Active";
    
    // Initialize Proof Stamp event collector
    collector = new EventCollector();
    
    Word.run(async (context) => {
      await collector.registerHooks(context);
    });

    // Note: Global DocumentBeforeSave registration requires item-level permissions.
    // Explicit trigger used for demonstration purposes.
    document.getElementById("btn-force-seal")!.onclick = async () => {
      document.getElementById("status-indicator")!.innerText = "Sealing document...";
      
      // Simulate save intercept
      await onBeforeSave({ preventDefault: false });
      
      document.getElementById("status-indicator")!.innerText = "Document sealed with ZK Proof!";
    };
  }
});
