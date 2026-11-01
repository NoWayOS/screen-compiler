// Synthetic Screen Compiler fixture: step 3 of 3 of the generic lost
// property form. The answers to check, in a scrolling view; a checkbox the
// person must tick, with its error; and a way back to change them - eleven
// answers, more than the view shows at 800 x 480, so it scrolls. It
// converts since the scroll view was built; `make contract-check`
// draws it unscrolled and a page down. Invented for Screen Compiler's tests; not
// taken from any product, form, customer or person.
import { Button, Checkbox, Column, Row, Screen, ScrollView, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type ReviewProps = {
  kind: string;
  when: string;
  place: string;
  colour: string;
  brand: string;
  description: string;
  name: string;
  phone: string;
  textMe: string;
  email: string;
  contactBy: string;
  confirmError: string;
  onBack: () => void;
  onCancel: () => void;
  onSend: () => void;
};

export function ReviewStep(props: ReviewProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="progress">Step 3 of 3</Text>
        <Text id="heading">Check your report</Text>
        <ScrollView id="answers">
          <Column gap={GAP}>
            <Row gap={GAP}>
              <Text>Item:</Text>
              <Text>{props.kind}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Lost:</Text>
              <Text>{props.when}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Where:</Text>
              <Text>{props.place}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Colour:</Text>
              <Text>{props.colour}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Brand:</Text>
              <Text>{props.brand}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Description:</Text>
              <Text>{props.description}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Name:</Text>
              <Text>{props.name}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Phone:</Text>
              <Text>{props.phone}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Text message:</Text>
              <Text>{props.textMe}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Email:</Text>
              <Text>{props.email}</Text>
            </Row>
            <Row gap={GAP}>
              <Text>Contact by:</Text>
              <Text>{props.contactBy}</Text>
            </Row>
          </Column>
        </ScrollView>
        <Checkbox id="confirm" label="These details are correct" />
        {props.confirmError && <Text>{props.confirmError}</Text>}
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Change</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="send" onPress={props.onSend}>
            Send
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
