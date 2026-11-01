// Synthetic Screen Compiler fixture: a short, generic two-field form, each field
// with a line beneath it for the errors the application finds.
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, FieldError, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP_FIELD = 12;
const GAP_ACTIONS = 16;
const EDGE = 32;

interface DetailsFormProps {
  onBack: () => void;
  onContinue: () => void;
}

export default function DetailsForm(props: DetailsFormProps) {
  return (
    <Screen>
      <Column gap={GAP_FIELD} padding={EDGE}>
        <Text id="heading">Your details</Text>
        <Text>Name</Text>
        <TextInput id="fullName" placeholder="First and last name" />
        <FieldError for="fullName" />
        <Text>Email address</Text>
        <TextInput id="email" placeholder="name@example.com" />
        <FieldError for="email" />
        <Spacer size={GAP_ACTIONS} />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
          <Spacer />
          <Button id="submit" onPress={props.onContinue}>
            Continue
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
