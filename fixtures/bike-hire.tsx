// Synthetic Screen Compiler fixture: a bike hire kiosk's screen, written for
// contract v13's conditions. The application sets the step the rider is on -
// choosing a bike, reviewing the hire or paying - and the screen shows that
// step's content, each `props.step === "…"` one case of one switch it sets;
// a note shows while the rider is not signed in, `!props.signedIn`. Written
// for Screen Compiler's tests; not taken from any product.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

type BikeHireProps = {
  step: string;
  signedIn: boolean;
  bike: string;
  onBack: () => void;
  onNext: () => void;
};

export function BikeHire(props: BikeHireProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="heading">Hire a bike</Text>
        {props.step === "choose" && <Text>Choose a bike from the stand.</Text>}
        {props.step === "review" && (
          <Row gap={GAP}>
            <Text>Your bike:</Text>
            <Text>{props.bike}</Text>
          </Row>
        )}
        {props.step === "pay" ? (
          <Text>Tap your card on the reader.</Text>
        ) : (
          <Text>You pay at the last step.</Text>
        )}
        {!props.signedIn && <Text>Sign in to keep a receipt.</Text>}
        <Spacer />
        <Row gap={GAP}>
          <Button onPress={props.onBack}>Back</Button>
          <Spacer />
          <Button onPress={props.onNext}>Next</Button>
        </Row>
      </Column>
    </Screen>
  );
}
