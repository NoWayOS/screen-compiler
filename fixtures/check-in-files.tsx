// Synthetic Screen Compiler fixture: check-in.tsx built of parts imported
// from files beside it (parts/visitor.tsx, and the design tokens in
// parts/theme.tsx), which Screen Compiler reads and expands at build time.
// It must convert to exactly what check-in.tsx does, and draw exactly its
// approved image. Written for Screen Compiler's tests; not taken from any
// product.
import { Card, Column, Screen, Text } from "nowayos:kit";
import { EDGE, GAP } from "./parts/theme";
import { Bar, Choices, Note, Question, type CheckInProps } from "./parts/visitor";

export function CheckIn(props: CheckInProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <Bar title="Visitor check-in" onHome={props.onHome} />
        <Column gap={GAP} padding={EDGE}>
          <Card gap={GAP} padding={EDGE}>
            <Question />
            <Choices onCancel={props.onCancel} onBack={props.onBack} onCheckIn={props.onCheckIn} />
          </Card>
          <Note tone="success">
            <Text>Your host has been told you are here.</Text>
          </Note>
          <Note tone="caution">
            <Text>Please wear your badge at all times.</Text>
          </Note>
          <Note tone="warning">
            <Text weight="bold">The lift on this floor is out of order.</Text>
          </Note>
        </Column>
      </Column>
    </Screen>
  );
}
