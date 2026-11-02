// Synthetic Screen Compiler fixture: check-in.tsx built of parts - the
// screen's own components and a fragment - which Screen Compiler expands
// at build time. It must convert to exactly what check-in.tsx does, and
// draw exactly its approved image. Written for Screen Compiler's tests;
// not taken from any product.
import { Button, Card, Column, HeaderBar, Notice, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 12;
const TIGHT = 8;

type CheckInProps = {
  onHome: () => void;
  onCancel: () => void;
  onBack: () => void;
  onCheckIn: () => void;
};

type BarProps = { title: string; onHome: () => void };

// A header bar with its title and Home.
function Bar({ title, onHome }: BarProps) {
  return (
    <HeaderBar gap={GAP} padding={EDGE}>
      <Text role="heading">{title}</Text>
      <Spacer />
      <Button variant="secondary" onPress={onHome}>
        Home
      </Button>
    </HeaderBar>
  );
}

// A notice in a tone, holding what it is given.
const Note = (props: { tone: "success" | "caution" | "warning"; children: JSX.Element }) => (
  <Notice tone={props.tone} padding={TIGHT}>
    {props.children}
  </Notice>
);

// The three choices: two buttons on the right, a quiet one on the left.
function Choices({ onCancel, onBack, onCheckIn }: Omit<CheckInProps, "onHome">) {
  return (
    <Row gap={GAP}>
      <Button variant="quiet" onPress={onCancel}>
        Cancel
      </Button>
      <Spacer />
      <Button variant="secondary" onPress={onBack}>
        Back
      </Button>
      <Button onPress={onCheckIn}>Check in</Button>
    </Row>
  );
}

// The question, as a fragment: its children take its place in the card.
function Question() {
  return (
    <>
      <Text role="title">Is this you?</Text>
      <Text>Sam Taylor, visiting the second floor.</Text>
    </>
  );
}

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
