// Synthetic Screen Compiler fixture: the parts check-in-files.tsx imports -
// a header bar, a notice, the choices and a fragment - in a file of their
// own beside it, with no screen. Written for Screen Compiler's tests; not
// taken from any product.
import { Button, HeaderBar, Notice, Row, Spacer, Text } from "nowayos:kit";
import { EDGE, GAP, TIGHT } from "./theme";

export type CheckInProps = {
  onHome: () => void;
  onCancel: () => void;
  onBack: () => void;
  onCheckIn: () => void;
};

type BarProps = { title: string; onHome: () => void };

// A header bar with its title and Home.
export function Bar({ title, onHome }: BarProps) {
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
export const Note = (props: { tone: "success" | "caution" | "warning"; children: JSX.Element }) => (
  <Notice tone={props.tone} padding={TIGHT}>
    {props.children}
  </Notice>
);

// The three choices: two buttons on the right, a quiet one on the left.
export function Choices({ onCancel, onBack, onCheckIn }: Omit<CheckInProps, "onHome">) {
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
export function Question() {
  return (
    <>
      <Text role="title">Is this you?</Text>
      <Text>Sam Taylor, visiting the second floor.</Text>
    </>
  );
}
