// Synthetic Screen Compiler fixture: a generic help screen whose paragraphs
// mix weights, italics, a theme colour, code and links (contract v21).
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, Screen, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 32;

interface HelpProps {
  onGuide: () => void;
  onCommands: () => void;
  onBack: () => void;
}

export default function Help(props: HelpProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="title">Help</Text>
        <Text>
          Press <strong>Save</strong> to keep your work, or read <a onClick={props.onGuide}>the guide</a> first.{" "}
          <em>Unsaved changes are lost</em> when the session ends.
        </Text>
        <Card padding={GAP}>
          <Text>
            Type <code>help --all</code> for <a onClick={props.onCommands}><strong>every command</strong></a>;{" "}
            <span style={{ color: "error" }}>errors show in red</span>.
          </Text>
        </Card>
        <Button variant="secondary" onPress={props.onBack}>
          Back
        </Button>
      </Column>
    </Screen>
  );
}
