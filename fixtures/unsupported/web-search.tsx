// Synthetic Screen Compiler fixture: a web-style React screen that uses constructs
// outside subset v0. `screen-compiler check` must report each one with its position
// and produce no output. Written for Screen Compiler's tests.
import { useState } from "react";
import { Button, Column, Screen, Text } from "nowayos:kit";
import "./search.css";

const tokens = { space: { md: 16 } };

export function SearchScreen({ onSearch, results }: SearchProps) {
  const [query, setQuery] = useState("");
  return (
    <Screen>
      <Column gap="16px" style={{ background: "#ffffff" }}>
        <h1 className="title">Search</h1>
        <Text>Results for {query}</Text>
        {results.map((r) => <Text key={r.id}>{r.label}</Text>)}
        {query.length > 0 && <Button onPress={() => setQuery("")}>Clear</Button>}
        <Button onClick={onSearch} disabled>
          Search
        </Button>
        <Text>&copy; Example Services</Text>
      </Column>
    </Screen>
  );
}
