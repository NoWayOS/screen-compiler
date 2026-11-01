// Synthetic Screen Compiler fixture: booking a visit, written for subset v1's
// form controls - text inputs with a length limit and an input
// mode, a radio group and a checkbox, each a field the application reads
// by number - with the phone number's error a group holding a slot.
// Written for Screen Compiler's tests; not taken from any product.
import {
  Button,
  Checkbox,
  Column,
  RadioGroup,
  Row,
  Screen,
  Spacer,
  Text,
  TextInput,
} from "nowayos:kit";

const GAP = 8;
const EDGE = 24;
const NAME_LEN = 32;

type VisitBookingProps = {
  phoneError: string;
  onBack: () => void;
  onBook: () => void;
};

export function VisitBooking(props: VisitBookingProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Book a visit</Text>
        <Row gap={GAP}>
          <TextInput id="name" placeholder="Your name" maxLength={NAME_LEN} />
          <TextInput id="phone" placeholder="Phone, digits only" inputMode="tel" maxLength={15} />
        </Row>
        {props.phoneError && <Text>{props.phoneError}</Text>}
        <TextInput id="email" placeholder="Email (optional)" inputMode="email" />
        <Text>When suits you?</Text>
        <RadioGroup id="when" options={["Morning", "Afternoon", "Evening"]} />
        <Checkbox id="remind" label="Remind me the day before" />
        <Spacer />
        <Row gap={GAP}>
          <Button onPress={props.onBack}>Back</Button>
          <Spacer />
          <Button id="book" onPress={props.onBook}>
            Book
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
