// Synthetic Screen Compiler fixture: a visitor's check-in, written for the look's
// buttons and surfaces. A header bar with Home on it, a card
// holding the question and the three button variants - Cancel quiet, Back
// secondary, Check in primary - and a notice in each tone. Written for
// Screen Compiler's tests; not taken from any product.
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

export function CheckIn(props: CheckInProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <HeaderBar gap={GAP} padding={EDGE}>
          <Text role="heading">Visitor check-in</Text>
          <Spacer />
          <Button variant="secondary" onPress={props.onHome}>
            Home
          </Button>
        </HeaderBar>
        <Column gap={GAP} padding={EDGE}>
          <Card gap={GAP} padding={EDGE}>
            <Text role="title">Is this you?</Text>
            <Text>Sam Taylor, visiting the second floor.</Text>
            <Row gap={GAP}>
              <Button variant="quiet" onPress={props.onCancel}>
                Cancel
              </Button>
              <Spacer />
              <Button variant="secondary" onPress={props.onBack}>
                Back
              </Button>
              <Button onPress={props.onCheckIn}>Check in</Button>
            </Row>
          </Card>
          <Notice tone="success" padding={TIGHT}>
            <Text>Your host has been told you are here.</Text>
          </Notice>
          <Notice tone="caution" padding={TIGHT}>
            <Text>Please wear your badge at all times.</Text>
          </Notice>
          <Notice tone="warning" padding={TIGHT}>
            <Text weight="bold">The lift on this floor is out of order.</Text>
          </Notice>
        </Column>
      </Column>
    </Screen>
  );
}
