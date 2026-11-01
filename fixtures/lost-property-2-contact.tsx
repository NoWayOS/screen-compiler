// Synthetic Screen Compiler fixture: step 2 of 3 of the generic lost
// property form. Three text fields - a name, a phone number typed on the
// on-screen keyboard's digits and an email address on its address layout -
// each with a length limit and a line beneath it for the errors the
// application finds - and a checkbox. With the keyboard and the
// buttons below them, the fields have too little room at 800 x 480 to show
// at once, so they scroll; focus moving to a field scrolls it into view. It
// converts since the keyboard was built; `make contract-check`
// draws it once for each of the keyboard's layouts.
// Invented for Screen Compiler's tests; not taken from any product, form,
// customer or person.
import {
  Button,
  Checkbox,
  Column,
  FieldError,
  Keyboard,
  Row,
  Screen,
  ScrollView,
  Spacer,
  Text,
  TextInput,
} from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 16;

type ContactProps = {
  onBack: () => void;
  onNext: () => void;
};

export function ContactStep(props: ContactProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="progress">Step 2 of 3</Text>
        <Text id="heading">How can we contact you?</Text>
        <ScrollView id="fields">
          <Column gap={GAP}>
            <Text>Your name</Text>
            <TextInput id="name" placeholder="First and last name" maxLength={40} />
            <FieldError for="name" />
            <Text>Phone number</Text>
            <TextInput id="phone" placeholder="Digits only" inputMode="tel" maxLength={15} />
            <FieldError for="phone" />
            <Text>Email address (optional)</Text>
            <TextInput id="email" placeholder="name@example.com" inputMode="email" maxLength={64} />
            <FieldError for="email" />
            <Checkbox id="textMe" label="Send me a text message if it is found" />
          </Column>
        </ScrollView>
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
        <Keyboard />
      </Column>
    </Screen>
  );
}
