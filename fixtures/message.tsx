// Synthetic Screen Compiler fixture: a generic message screen with layers over it
// (contract v20) - a menu beside More, a tooltip on Delete, a popover beside Help
// and a dialog confirming a delete.
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, Dialog, HeaderBar, Menu, Popover, Row, Screen, Spacer, Text, Tooltip } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

interface MessageProps {
  menuOpen: boolean;
  helpOpen: boolean;
  confirming: boolean;
  onMore: () => void;
  onRename: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onHelp: () => void;
  onKeep: () => void;
  onConfirm: () => void;
}

export default function Message(props: MessageProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <HeaderBar gap={GAP} padding={GAP}>
          <Text role="heading">A message</Text>
          <Spacer />
          <Button id="more" variant="secondary" onPress={props.onMore}>
            More
          </Button>
          <Menu for="more" open={props.menuOpen}>
            <Column gap={4}>
              <Button variant="quiet" onPress={props.onRename}>
                Rename
              </Button>
              <Button variant="quiet" onPress={props.onArchive}>
                Archive
              </Button>
            </Column>
          </Menu>
        </HeaderBar>
        <Column gap={GAP} padding={EDGE}>
          <Text lines={3}>A message of a few lines, which the person may delete, rename or keep.</Text>
          <Row gap={GAP}>
            <Button id="delete" variant="secondary" onPress={props.onDelete}>
              Delete
            </Button>
            <Tooltip for="delete">Deletes the message for good</Tooltip>
            <Button id="help" variant="quiet" onPress={props.onHelp}>
              Help
            </Button>
            <Popover for="help" open={props.helpOpen}>
              <Text lines={2}>Rename gives it a name of your own; Archive keeps it out of the way.</Text>
            </Popover>
          </Row>
        </Column>
        <Dialog open={props.confirming}>
          <Column gap={GAP}>
            <Text role="heading">Delete this message?</Text>
            <Text>It cannot be brought back.</Text>
            <Row gap={GAP}>
              <Button variant="secondary" onPress={props.onKeep}>
                Keep
              </Button>
              <Button onPress={props.onConfirm}>Delete</Button>
            </Row>
          </Column>
        </Dialog>
      </Column>
    </Screen>
  );
}
