import React from "react";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";

export default function QueryEditor() {
  const [query, setQuery] = React.useState("");

  return (
    <div className="flex h-full flex-col gap-2 p-4">
      <Textarea
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Escreva sua SQL aqui…"
        className="flex-1 min-h-0 resize-none font-mono"
      />

      <div className="flex justify-end">
        <Button>Run</Button>
      </div>
    </div>
  );
}
