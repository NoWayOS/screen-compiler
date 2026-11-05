// Synthetic Screen Compiler fixture: a short, generic feedback form - a
// name, and a comment of several lines in a text area (contract v19).
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, FieldError, Row, Screen, Spacer, Text, TextArea, TextInput } from "nowayos:kit";

const GAP_FIELD = 12;
const EDGE = 32;
const MOST_COMMENT = 1000;

interface FeedbackProps {
  onBack: () => void;
  onSend: () => void;
}

export default function Feedback(props: FeedbackProps) {
  return (
    <Screen>
      <Column gap={GAP_FIELD} padding={EDGE}>
        <Text role="title">Tell us how it went</Text>
        <Text>Your name (optional)</Text>
        <TextInput id="name" placeholder="First name" maxLength={32} />
        <Text>Your comments</Text>
        <TextArea id="comments" rows={5} maxLength={MOST_COMMENT} placeholder="What went well, and what could be better?" />
        <FieldError for="comments" />
        <Spacer />
        <Row gap={GAP_FIELD}>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Spacer />
          <Button onPress={props.onSend}>Send</Button>
        </Row>
      </Column>
    </Screen>
  );
}
