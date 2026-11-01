// Synthetic Screen Compiler fixture: step 1 of 3 of a generic form for
// reporting lost property at a public self-service station. The three steps
// together use every control UI Runtime lists for form-heavy kiosks. This one:
// a list picker with more options than a radio group holds, a radio group,
// and an error beside each field. It converts since the list picker was
// built. Invented for Screen Compiler's tests; not taken from any product,
// form, customer or person.
import { Button, Column, RadioGroup, Row, Screen, Select, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type LostItemProps = {
  kindError: string;
  whenError: string;
  onCancel: () => void;
  onNext: () => void;
};

export function LostItemStep(props: LostItemProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="progress">Step 1 of 3</Text>
        <Text id="heading">What did you lose?</Text>
        <Text>Kind of item</Text>
        <Select
          id="kind"
          placeholder="Choose one"
          rows={4}
          options={[
            "Bag or case",
            "Phone",
            "Keys",
            "Wallet or purse",
            "Glasses",
            "Umbrella",
            "Coat or other clothing",
            "Jewellery or watch",
            "Book or papers",
            "Toy",
            "Something else",
          ]}
        />
        {props.kindError && <Text>{props.kindError}</Text>}
        <Text>When did you lose it?</Text>
        <RadioGroup id="when" options={["Today", "Yesterday", "Earlier this week", "Before that"]} />
        {props.whenError && <Text>{props.whenError}</Text>}
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
