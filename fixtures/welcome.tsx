// Synthetic Screen Compiler fixture: a generic welcome screen for a self-service
// station. Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, Screen, Spacer, Text } from "nowayos:kit";

export const SPACE_SM = 8;
export const SPACE_LG = 24;

type WelcomeProps = {
  onStart: () => void;
  onHelp: () => void;
};

export function WelcomeScreen({ onStart, onHelp }: WelcomeProps) {
  return (
    <Screen>
      <Column gap={SPACE_SM} padding={SPACE_LG}>
        <Text id="title">Welcome</Text>
        <Text>Touch Start to begin.</Text>
        <Text>This station keeps nothing once you have finished.</Text>
        <Spacer />
        <Button id="start" onPress={onStart}>
          Start
        </Button>
        <Button onPress={onHelp}>Help &amp; accessibility</Button>
      </Column>
    </Screen>
  );
}
