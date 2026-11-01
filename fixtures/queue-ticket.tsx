// Synthetic Screen Compiler fixture: a queue ticket, written for the type scale.
// The ticket's number is the display role - digits, capitals and
// a few signs, read across a room - and comes from the application as a
// slot; the title, a label, body text in Regular and Bold, and a caption
// each name their role. Written for Screen Compiler's tests; not taken from any
// product.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

type QueueTicketProps = {
  ticket: string;
  onDone: () => void;
};

export function QueueTicket(props: QueueTicketProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="title">You are in the queue</Text>
        <Text role="label" weight="bold">
          Your number
        </Text>
        <Text id="ticket" role="display">
          {props.ticket}
        </Text>
        <Text>We will call your number when a desk is free.</Text>
        <Text weight="bold">Please stay in the waiting area.</Text>
        <Spacer />
        <Row gap={GAP}>
          <Text role="caption">Numbers are called in order.</Text>
          <Spacer />
          <Button onPress={props.onDone}>Done</Button>
        </Row>
      </Column>
    </Screen>
  );
}
